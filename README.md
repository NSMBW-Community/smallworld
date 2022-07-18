# smallworld

*...though the mountains divide and the oceans are wide...*

**smallworld** is a little tool that can create **region-free** `openingTitle.arc` files for *New Super Mario Bros. Wii* **with no code hacks** and minimal file-size overhead (about 0.3%). It can also convert an `openingTitle.arc` from one region to another.

## Usage

Download the latest release package for your OS from [the releases page](https://github.com/RoadrunnerWMC/smallworld/releases). Each one includes several builds; pick the one that matches your system's architecture (if you're not sure, `x86_64` is the most common).

### Quick overview of the CLI

Replace "`smallworld`" in the commands below with the full name of the binary you've chosen.

To make an `openingTitle.arc` region-free in-place:

```sh
smallworld openingTitle.arc
```

To convert an `openingTitle.arc` to a particular single region (say, Japanese):

```sh
smallworld --to J openingTitle.arc
```

To save to a different filename instead of overwriting the input file:

```sh
smallworld -o modified.arc openingTitle.arc
```

To ignore conflicting copies of a file and just pick the EU one\*:

```sh
smallworld --ignore-conflicts openingTitle.arc
```

To see full usage information:

```sh
smallworld --help
```

*\*By default. You can use `--from` to configure how conflicts are resolved; see `--help` for more details.*


## License

GNU GPL v3. See the "LICENSE" file for more information.


## FAQ

**What?** `openingTitle.arc` is the archive file in NSMBW that contains the logo image shown on the title screen, and the associated layout and animation files. All of the files within it have names that vary slightly from region to region -- for example, the layout file is `openingTitle_US_00.brlyt` in the North American release, `openingTitle_EU_00.brlyt` in the international ("EU") release, and `openingTitle_13.brlyt` in the Japanese release. These filenames are hardcoded in the code; thus, each `openingTitle.arc` is tied to a single region, and would crash the game if used in another.

Since the different versions of NSMBW are very similar overall, it's customary for mods to support several of them, usually at least 3 (US, EU, JP). The traditional way to create custom versions of `openingTitle.arc` has been to manually create multiple copies of it, one per region, and let the game load the correct one depending on which game region is found at runtime (`openingTitle.arc` is also *located at* different paths in different regions, too).

smallworld provides a better solution, in the form of low-overhead, region-free `openingTitle.arc`s. This is done by adding redundant filenames to the archive's filename table, which all point to the same internal file data. This way, no matter which filenames the game uses, the lookups will always succeed.

**Why should I use this?** It's important to deal with `openingTitle.arc` in one way or another. To support only one region in your mod would be to lock out a large percentage of potential players.

The traditional approach to the `openingTitle.arc` problem has some downsides:

* Your mod needs to include multiple copies of `openingTitle.arc`, which are 99% identical.
* If you ever want to edit your logo again, you have to do the manual file-renaming process again afterward.
* You might make a typo or mistake (for example, it's easy to forget that the Japanese files are named "13" instead of "JP_00"), and unless you *have* every version of the game available to test with (and can be bothered to actually do that), you would have no way of noticing.
* Most people don't bother including Korean and Taiwanese `openingTitle.arc`s, because those versions of the game are rather obscure and it just takes longer to make even more copies of the file.

Instead of all that, you can just let smallworld take care of it for you. Quickly and easily create a single region-free `openingTitle.arc`, put it in your mod, and you'll never have to bother doing it the manual way ever again.

**Why not use code hacks to make the filenames consistent instead?** If you prefer that, feel free. This is just a different solution that doesn't require any code hacks.

**Does it work with Newer SMBW?** Yes.

**If the file is at different paths in different regions, how can they share a single file in a mod?** That can be done through the Riivolution XML (you are using Riivolution, right?). Add an entry for each region's `openingTitle` folder, and point them all to a single shared folder in your mod.

**How do I *edit* a region-free `openingTitle.arc`? There are so many duplicate files in it!** You have two options:

* Three-step process: run it through smallworld with the `--to` option to convert to a single region (pick any of them), edit the file as usual, and finally use smallworld to make it region-free again.
* Two-step process, slightly riskier if you're not careful: edit the "EU" files, then run the arc file through smallworld again with the "`--ignore-conflicts`" flag to re-apply the file deduplication. Normally, if the files for each region are different at all, smallworld will play it safe and fail with an error message. `--ignore-conflicts` causes it to ignore that and just pick the EU versions (by default -- see "`--from`" in the `--help` output for more information) if they exist. So if you do this, **you MUST edit the "EU" files specifically** or else it'll choose the **old** versions of your files and discard the new ones!

**I edited my region-free `openingTitle.arc` in another application, and when I saved, it's suddenly 6x larger! Help!** smallworld makes the redundant filenames point to *exactly* the same data in the arc file. Other applications don't do this, so when re-saving, they'll create separate copies of the data for each filename, ballooning the overall file size. To fix it, **first make sure that your edits are on the "EU" versions of the files,** then run the file through smallworld again with the "`--ignore-conflicts`" flag. (Also see the previous question.)

**Why doesn't it rename the TPL file?** `openingTitle.arc` contains BRLAN files (animations), a BRLYT file (layout), and a TPL file (image). All of these have different filenames in every region.\* So why does smallworld not touch the TPL at all?

The BRLAN and BRLYT filenames are referenced by hardcoded strings in the game code, so they need to be renamed for every region. The TPL filename, on the other hand, is only referenced by the BRLYT file data. As such, not only is renaming it unnecessary, it's actually **dangerous** because it'll break this reference unless the BRLYT file is also updated to match the new filename.

This would also require storing separate BRLYT files per region instead of using one shared one, and smallworld would need to incorporate code for editing BRLYTs. It's much easier to just leave the TPL filename alone, and it works perfectly well that way.

*\*Except for the US and EU regions, which happen to use the same name for the TPL.*

**Why did you write this in Rust??** I wanted to practice it, and this seemed like a nice project to try it on.
