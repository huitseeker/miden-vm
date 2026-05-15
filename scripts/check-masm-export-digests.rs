#!/usr/bin/env -S cargo +nightly -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
miden-assembly-current = { package = "miden-assembly", path = "../crates/assembly" }
miden-mast-package-current = { package = "miden-mast-package", path = "../crates/mast-package" }
miden-package-registry-current = { package = "miden-package-registry", path = "../crates/package-registry" }

miden-assembly-previous = { package = "miden-assembly", git = "https://github.com/0xMiden/miden-vm", tag = "v0.22.3" }
miden-mast-package-previous = { package = "miden-mast-package", git = "https://github.com/0xMiden/miden-vm", tag = "v0.22.3" }
miden-package-registry-previous = { package = "miden-package-registry", git = "https://github.com/0xMiden/miden-vm", tag = "v0.22.3" }
---

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process,
};

type Exports = BTreeMap<String, String>;

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let input = env::args().nth(1).map(PathBuf::from).ok_or_else(|| {
        "usage: check-masm-export-digests.rs <miden-project.toml|project-dir|package.masp>"
            .to_string()
    })?;

    let previous = previous::collect_exports(&input)?;
    let current = current::collect_exports(&input)?;
    compare_exports(previous, current)
}

fn compare_exports(previous: Exports, current: Exports) -> Result<(), String> {
    let mut status = Ok(());
    let export_names = previous.keys().chain(current.keys()).cloned().collect::<BTreeSet<_>>();

    for name in export_names {
        match (previous.get(&name), current.get(&name)) {
            (Some(previous_digest), Some(current_digest)) if previous_digest == current_digest => {
                println!("{name} {current_digest}");
            },
            (Some(previous_digest), Some(current_digest)) => {
                eprintln!(
                    "::error::export digest changed for {name}: previous={previous_digest}, current={current_digest}",
                );
                status = Err("procedure export digests changed".to_string());
            },
            (Some(previous_digest), None) => {
                eprintln!("::error::export removed: {name} previous={previous_digest}");
                status = Err("procedure exports changed".to_string());
            },
            (None, Some(current_digest)) => {
                eprintln!("::error::export added: {name} current={current_digest}");
                status = Err("procedure exports changed".to_string());
            },
            (None, None) => unreachable!("name came from at least one side"),
        }
    }

    status
}

mod current {
    use super::*;
    use miden_assembly_current::{Assembler, ProjectTargetSelector};
    use miden_mast_package_current::{Package, PackageExport};
    use miden_package_registry_current::InMemoryPackageRegistry;

    pub fn collect_exports(input: &Path) -> Result<Exports, String> {
        if input.extension().and_then(|extension| extension.to_str()) == Some("masp") {
            let package = read_package(input)?;
            return collect_package_exports(&package);
        }

        let mut store = InMemoryPackageRegistry::default();
        let mut project =
            Assembler::default().for_project_at_path(input, &mut store).map_err(|err| {
                format!("current: failed to load project '{}': {err}", input.display())
            })?;
        let package =
            project.assemble(ProjectTargetSelector::Library, "release").map_err(|err| {
                format!("current: failed to assemble project '{}': {err}", input.display())
            })?;

        collect_package_exports(package.as_ref())
    }

    fn read_package(path: &Path) -> Result<Package, String> {
        use miden_assembly_current::serde::Deserializable;

        let bytes = fs::read(path).map_err(|err| {
            format!("current: failed to read package '{}': {err}", path.display())
        })?;
        Package::read_from_bytes(&bytes).map_err(|err| {
            format!("current: failed to deserialize package '{}': {err}", path.display())
        })
    }

    fn collect_package_exports(package: &Package) -> Result<Exports, String> {
        Ok(package
            .manifest
            .exports()
            .filter_map(|export| match export {
                PackageExport::Procedure(procedure) => {
                    Some((procedure.path.to_string(), procedure.digest.to_string()))
                },
                PackageExport::Constant(_) | PackageExport::Type(_) => None,
            })
            .collect())
    }
}

mod previous {
    use super::*;
    use miden_assembly_previous::{Assembler, ProjectTargetSelector};
    use miden_mast_package_previous::{Package, PackageExport};
    use miden_package_registry_previous::InMemoryPackageRegistry;

    pub fn collect_exports(input: &Path) -> Result<Exports, String> {
        if input.extension().and_then(|extension| extension.to_str()) == Some("masp") {
            let package = read_package(input)?;
            return collect_package_exports(&package);
        }

        let mut store = InMemoryPackageRegistry::default();
        let mut project =
            Assembler::default().for_project_at_path(input, &mut store).map_err(|err| {
                format!("previous: failed to load project '{}': {err}", input.display())
            })?;
        let package =
            project.assemble(ProjectTargetSelector::Library, "release").map_err(|err| {
                format!("previous: failed to assemble project '{}': {err}", input.display())
            })?;

        collect_package_exports(package.as_ref())
    }

    fn read_package(path: &Path) -> Result<Package, String> {
        use miden_assembly_previous::serde::Deserializable;

        let bytes = fs::read(path).map_err(|err| {
            format!("previous: failed to read package '{}': {err}", path.display())
        })?;
        Package::read_from_bytes(&bytes).map_err(|err| {
            format!("previous: failed to deserialize package '{}': {err}", path.display())
        })
    }

    fn collect_package_exports(package: &Package) -> Result<Exports, String> {
        Ok(package
            .manifest
            .exports()
            .filter_map(|export| match export {
                PackageExport::Procedure(procedure) => {
                    Some((procedure.path.to_string(), procedure.digest.to_string()))
                },
                PackageExport::Constant(_) | PackageExport::Type(_) => None,
            })
            .collect())
    }
}
