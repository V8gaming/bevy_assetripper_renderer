use std::{path::{PathBuf, Path}, fs::File, io::{Read, self, Write}, process::exit};
use blake3::{Hasher, Hash};
use arrayvec::ArrayString;
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use signal_hook::consts::signal::*;
use signal_hook::flag;
use indicatif::{ProgressBar, ProgressStyle};
use walkdir::WalkDir;

pub struct Materials {
    directory: String,
    output_directory: String,
    guids: Vec<(String, String)>,
    completed: bool,
    step: usize,
    current_material: usize,
    materials: Vec<PathBuf>,
    mesh_files: Vec<PathBuf>,
    total_materials: usize,
    hash: Option<ArrayString<64>>,
    is_terminating: Arc<AtomicBool>,
}

impl Materials {
    fn check(&mut self) -> Result<usize, Box<dyn Error>> {
        let directory = self.output_directory.clone();
        let toml = std::fs::File::open(format!("{}/log.toml", directory));
        match toml {
            Ok(_) => {
                let mut toml = toml.unwrap();
                let mut toml_string = String::new();
                toml.read_to_string(&mut toml_string).unwrap();
                let toml: Toml = toml::from_str(&toml_string).unwrap();
                let hash = hash_from_hex_str(&toml.header.hash).unwrap();
                // if version or hash don't match, restart
                // if completed, skip
                // if not completed, continue from toml.current
                if toml.header.version == env!("CARGO_PKG_VERSION") {
                    if hash == hash_directory(self.directory.clone()).unwrap() {     
                        if !toml.header.completed {
                            println!("Hashes match, but not completed, continuing");

                            self.current_material = toml.current;
                            self.materials = toml.data.materials;
                            self.total_materials = toml.total;
                            self.guids = toml.data.guids;
                            self.hash = Some(hash.to_hex());
                            self.step = toml.step;
                            return Ok(toml.step);

                        }
                        println!("Hashes match, completed, skipping");
                        Err("Hashes match, completed, skipping")?;
                    }
                    if toml.header.completed {
                        println!("Completed, skipping");
                        Err("Completed, skipping")?;
                    }
                    println!("Hashes don't match, restarting");
                    
                    Ok(0)
                } else {
                    println!("Version mismatch, restarting");
                    Ok(0)
                }
            

            }
            Err(_) => {
                println!("No log file found, starting from scratch");
                Ok(0)
            }
        }

    }
    pub fn from_dir(input: &str, output: &str) -> Materials {
        Materials {
            directory: input.to_string(),
            output_directory: output.to_string(),
            guids: Vec::new(),
            completed: false,
            step: 0,
            current_material: 0,
            materials: Vec::new(),
            mesh_files: Vec::new(),
            hash: None,
            total_materials: 0,
            is_terminating: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn run(mut self) -> Result<(), Box<dyn Error>> {
        flag::register(SIGINT, Arc::clone(&self.is_terminating))?;
        flag::register(SIGTERM, Arc::clone(&self.is_terminating))?;
        match self.check()? {
            0 => {
                self.linker()?;
                self.parse_materials()?;
                self.log_progress()?;
            }
            1 => {
                self.parse_materials()?;
                self.log_progress()?;
            }
            _ => {
                println!("Invalid step");
                exit(1);
            }

        }
        Ok(())
    }
    fn linker(&mut self) -> Result<(), Box<dyn Error>> {
        println!("Linking materials");
        let directory = self.directory.clone();

        let mut folders_vec: Vec<PathBuf> = Vec::new();
        // find all folders in directory, recursively
        for entry in WalkDir::new(directory).into_iter().filter_entry(|e| e.file_type().is_dir()) {
            match entry {
                Ok(dir) => {
                    folders_vec.push(dir.into_path());
                }
                Err(e) => println!("Error: {}", e),
            }
        }
    
        
        let mut vec: Vec<(String, String)> =  Vec::new();
        // only get files that end with .mat
        let mut material_files = Vec::new();
        let mut mesh_files = Vec::new();


        // only get files that end with .meta
        let meta_files_vec: Vec<Vec<(String, String)>> = folders_vec
            .iter()
            .map(|folder| {
                folder
                    .read_dir()
                    .unwrap()
                    .filter_map(|entry| {
                        let entry = entry.unwrap();
                        let path = entry.path();
                        if path.is_file() && path.extension().unwrap() == "meta" {
                            let file_path = path.to_str().unwrap();
                            let file_name = file_path.strip_suffix(".meta").unwrap();
                            Some((std::fs::read_to_string(entry.path()).unwrap(), file_name.to_string()))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .collect();

        for folder in folders_vec {
            
            let local_material_files = folder
            .read_dir()?
            .filter_map(|entry| {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_file() && path.extension().unwrap() == "mat" {
                    Some(path)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
            material_files.extend(local_material_files);
            let local_mesh_files = folder
            .read_dir()?
            .filter_map(|entry| {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_file() && path.extension().unwrap() == "glb" {
                    Some(path)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
            mesh_files.extend(local_mesh_files);



        }
        // get the line that starts with "guid:", and push to the hashmap,
        // the guid, and the file path, but with .meta suffix removed
        for meta_files in meta_files_vec {
            for (meta_file, path) in meta_files {
                let guid = meta_file
                    .lines()
                    .find(|line| line.starts_with("guid:"))
                    .unwrap()
                    .strip_prefix("guid: ")
                    .unwrap();
                //println!("{} {}", guid, path);
                vec.push((guid.to_string(), path.clone()));
            }
        }

        println!("{} materials found", material_files.len());
        self.total_materials = material_files.len();
        self.materials = material_files;
        self.mesh_files = mesh_files;
        self.guids = vec;
        self.step = 1;
        Ok(())
        
    }
    fn parse_material(&self, material: PathBuf) -> Result<(), Box<dyn Error>>{
        let lines = std::fs::read_to_string(material.clone())?;
        let mut names: Vec<String> =  Vec::new();
        let mut guids: Vec<Option<String>> =  Vec::new();
        for line in lines.clone().lines() {
            if line.trim_start().starts_with("m_Shader:") {
                names.push("Shader".to_string());
                let mut texture = line.trim()
                    .strip_prefix("m_Shader: {fileID: 4800000, guid: ");
                if texture.is_none() {
                    guids.push(None);
                    continue;
                }
                texture = texture.unwrap().strip_suffix(", type: 3}");  
                if texture.is_none() {
                    guids.push(None);
                    continue;
                }
                guids.push(Some(texture.unwrap().to_string()));
            }
/*             else if line.starts_with("m_Script:") {
                names.push("Script".to_string());
                let mut texture = line.trim()
                    .strip_prefix("m_Script: {fileID: 11500000, guid: ");
                if texture.is_none() {
                    guids.push(None);
                    continue;
                }
                texture = texture.unwrap().strip_suffix(", type: 3}");
                if texture.is_none() {
                    guids.push(None);
                    continue;
                }
                guids.push(Some(texture.unwrap().to_string()));
            } */
        }
        // get lines between "m_TexEnvs:" and "m_Floats:"

        let start = lines.find("m_TexEnvs:").unwrap() + "m_TexEnvs:".len();
        let end = lines.find("m_Floats:").unwrap();
        let texture_environments_block = &lines[start..end];

        for line in texture_environments_block.lines() {
            if line.is_empty() {
                continue;
            }

            if line.trim_start().starts_with('-') {
                let name = line.trim()
                .strip_prefix('-').unwrap().to_string()
                .strip_suffix(':').unwrap().to_string();
                names.push(name);
            }

            if line.trim_start().starts_with("m_Texture:") {
                let mut texture = line.trim()
                    .strip_prefix("m_Texture: {fileID: 2800000, guid: ");
                if texture.is_none() {
                    guids.push(None);
                    continue;
                }
                texture = texture.unwrap().strip_suffix(", type: 3}");
                if texture.is_none() {
                    guids.push(None);
                    continue;
                }
                guids.push(Some(texture.unwrap().to_string()));

            }
        }
        let mut removed = 0;
        for ((i, _name), guid) in names.clone().iter().enumerate().zip(guids.iter()) {
            if guid.is_none() {
                names.remove(i-removed);
                removed += 1;
                continue;
            }
            //println!("{}: {}", name, guid.clone().unwrap());
        }

        // match the guids to the paths
        //println!("{},{}", names.len(), guids.len());
        let mut textures: Vec<(String, String)> = Vec::new();
        for (name, guid) in names.iter().zip(guids.iter()) {
            if guid.is_none() {
                continue;
            }
            let guid = guid.clone().unwrap();
            let path = self.guids.iter().find(|(g, _)| g == &guid);
            if path.is_none() {
                println!("{}: not found", name);
                continue;
            }
            let path = path.unwrap().1.clone();
            let extention = path.split('.').last().unwrap();
            let name = format!("{}.{}",name.clone(), extention);
            textures.push((name.clone(), path.clone()));
            //println!("{}: {}", name, path);
        }
        // create a material folder with the same name as the material file, and copy the textures there
        let material_path = material.clone();
        let material_name = material.file_name().unwrap().to_str().unwrap();
        let material_name = material_name.strip_suffix(".mat").unwrap();

        // find mesh in self.mesh_files
        let split = material_name.split('_').collect::<Vec<&str>>();
        if split.len() <= 1 {
            return Ok(());
        }
        let contain_name: String = format!("_{}",material_name.split('_').collect::<Vec<&str>>()[1]);
        
        let mesh_path = self.mesh_files.iter().find(|path| path.to_str().unwrap().contains(contain_name.as_str()));

        if Path::new(&format!("{}/assets/{}", self.output_directory, material_name)).exists() {
            std::fs::remove_dir_all(&format!("{}/assets/{}", self.output_directory, material_name))?;
        }
        std::fs::create_dir_all(format!("{}/assets/{}", self.output_directory,material_name))?;
        // copy .mat file to the material folder
        std::fs::copy(material_path.clone(), format!("{}/assets/{}/{}.mat", self.output_directory, material_name, material_name))?;

        if let Some(path)  = mesh_path {
            let mesh_name = path.file_name().unwrap().to_str().unwrap();
            //println!("{}: {}", material_name, mesh_name);
            //println!("{:?}", mesh_path);
            std::fs::copy(path, format!("{}/assets/{}/{}", self.output_directory, material_name, mesh_name))?;
        }
        for (name, path) in textures {
            //println!("{}: {}", name, path);
            // copy the texture to the material folder
            std::fs::copy(path, format!("{}/assets/{}/{}", self.output_directory, material_name, name))?;
            
        };
        Ok(())

    }
    fn parse_materials(&mut self) -> Result<(), Box<dyn Error>> {
        println!("Parsing materials");
        let _ = std::fs::remove_dir_all(self.output_directory.clone());

        self.materials.sort_by_key(|path| path.clone());
        // bar with msg of current material and total materials
        let bar = ProgressBar::new(self.materials.len() as u64);
        let style = ProgressStyle::with_template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos:>7}/{len:7} ({eta}) {msg}");
        bar.set_style(style?);
        self.hash = Some(hash_directory(self.directory.clone())?.to_hex());
        for i in self.current_material..self.materials.len() {
            let material = self.materials[i].clone();
            if self.is_terminating.load(Ordering::Relaxed) {
                eprintln!("Interrupted! Exiting gracefully...");
                self.log_progress()?; // Make sure to log progress before exiting.
                exit(0);
            }
            Materials::parse_material(self, material.to_path_buf())?;
            //println!("{}", self.total_materials);
            let log_interval = 10f64.powi((self.total_materials as f64).log10() as i32 - 1);

            if self.current_material % log_interval as usize == 0 || self.completed { 
                //println!("{} of {} materials processed", self.current_material, self.total_materials);
                self.log_progress()?; // Log progress after processing each 10th material.
            }

            bar.set_message(
                material.file_name()
                .unwrap().to_str().unwrap()
                .strip_suffix(".mat").unwrap().to_string());
            bar.inc(1);
            self.current_material += 1;
        }
        self.completed = true;
        self.step = 2;
        self.log_progress()?;
        bar.finish();
        Ok(())
    }
    fn log_progress(&self) -> Result<(), Box<dyn Error>>{
        // if dir is None, dir is the current directory
        let file_path = format!("{}/log.toml", self.output_directory.clone());
        std::fs::File::create(&file_path)?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open(file_path)?;
        let header = Header {
            date_time: chrono::Local::now().to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            directory: self.directory.clone(),
            completed: self.completed,
            hash: self.hash.unwrap().to_string(),
        };

        let toml = Toml {
            header,
            step: self.step,
            current: self.current_material,
            total: self.total_materials,
            data: Data {
                materials: self.materials.clone(),
                guids: self.guids.clone(),
            }
        };
        let toml = toml::to_string(&toml)?;
        file.write_all(toml.as_bytes())?;
        
        Ok(())
        
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Toml {
    header: Header,
    step: usize,
    current: usize,
    total: usize,
    data: Data,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Header {
    date_time: String,
    version: String,
    directory: String,
    completed: bool,
    hash: String,
} 

#[derive(serde::Serialize, serde::Deserialize)]
struct Data {
    materials: Vec<PathBuf>,
    guids: Vec<(String, String)>,
    
}

fn hash_directory<P: AsRef<Path>>(path: P) -> io::Result<Hash> {
    let mut hasher = Hasher::new();
    for entry in WalkDir::new(path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let mut file = File::open(entry.path())?;
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)?;
            hasher.update(&contents);
            hasher.update(entry.file_name().to_string_lossy().as_bytes());
        }
    }
    Ok(hasher.finalize())
}

fn hash_from_hex_str(hex_str: &str) -> io::Result<Hash> {
    if hex_str.len() == 64 {
        let bytes: Vec<u8> = hex::decode(hex_str)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let byte_array: [u8; 32] = <std::vec::Vec<u8> as std::convert::TryInto<[u8; 32]>>::try_into(bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Hex string must represent 32 bytes."))?;
        Ok(Hash::from(byte_array))
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Hex string must represent 32 bytes.",
        ))
    }
}