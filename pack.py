"""Pack the mod into recipes.hmod (flat layout) and install it into the Horizon instance."""
import shutil, zipfile, pathlib
root = pathlib.Path(__file__).parent
files = ["horizon-mod.toml", "Cargo.toml"] + [p.relative_to(root).as_posix() for p in (root / "src").rglob("*.rs")]
out = root / "recipes.hmod"
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for f in files:
        z.write(root / f, f)
dest = pathlib.Path.home() / "AppData/Roaming/horizon/horizon-native-mods/recipes.hmod"
shutil.copyfile(out, dest)
print("installed", dest, files)
