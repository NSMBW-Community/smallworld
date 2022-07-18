#![feature(concat_bytes)]
#![allow(clippy::unnecessary_wraps)]

use std::process::Command;

use anyhow::Result;
use assert_cmd::{assert::OutputAssertExt, cargo::CommandCargoExt};
use assert_fs::{
    assert::PathAssert,
    fixture::{FileTouch, FileWriteBin},
    NamedTempFile,
};
use predicates::prelude::predicate;

const BIN_NAME: &str = "smallworld";

#[test]
fn test_file_doesnt_exist() -> Result<()> {
    let mut cmd = Command::cargo_bin(BIN_NAME)?;

    cmd.arg("test/file/doesnt/exist");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("couldn't open"));

    Ok(())
}

#[test]
fn test_zero_byte_file() -> Result<()> {
    let filepath = NamedTempFile::new("test.arc")?;
    filepath.touch()?;

    let mut cmd = Command::cargo_bin(BIN_NAME)?;

    cmd.arg(filepath.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("invalid U8 file"));

    Ok(())
}

#[test]
fn test_empty_u8_file() -> Result<()> {
    let filepath = NamedTempFile::new("test.arc")?;
    filepath.write_binary(concat_bytes!(
        b"U\xaa8-\0\0\0 \0\0\0\r\0\0\0@\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"\x01\0\0\0\0\0\0\0\0\0\0\x01\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    ))?;

    let mut cmd = Command::cargo_bin(BIN_NAME)?;
    cmd.arg(filepath.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("anim folder not found"));

    Ok(())
}

#[test]
fn test_arg_from() -> Result<()> {
    let filepath = NamedTempFile::new("test.arc")?;

    // {
    //     "arc": {
    //         "anim": {
    //             "openingTitle_EU_00_inPress.brlan": b"in_press_brlan_eu",
    //             "openingTitle_EU_00_inTitle.brlan": b"in_title_brlan_eu",
    //             "openingTitle_EU_00_loopPress.brlan": b"loop_press_brlan_eu",
    //             "openingTitle_EU_00_outPress.brlan": b"out_press_brlan_eu",
    //             "openingTitle_US_00_inPress.brlan": b"in_press_brlan",
    //             "openingTitle_US_00_inTitle.brlan": b"in_title_brlan",
    //             "openingTitle_US_00_loopPress.brlan": b"loop_press_brlan",
    //             "openingTitle_US_00_outPress.brlan": b"out_press_brlan",
    //             "openingTitle_CN_00_inPress.brlan": b"in_press_brlan",
    //             "openingTitle_CN_00_inTitle.brlan": b"in_title_brlan",
    //             "openingTitle_CN_00_loopPress.brlan": b"loop_press_brlan",
    //             "openingTitle_CN_00_outPress.brlan": b"out_press_brlan",
    //             "some other random thing": b"whatever"},
    //         "blyt": {
    //             "openingTitle_EU_00.brlyt": b"brlyt_eu",
    //             "openingTitle_US_00.brlyt": b"brlyt",
    //             "openingTitle_CN_00.brlyt": b"brlyt"},
    //         "timg": {
    //             "wiiMario_Title_logo_local_00.tpl": b"tpl",
    //             "wiiMario_Title_logo_CN.tpl": b"tpl"}}}
    filepath.write_binary(concat_bytes!(
        b"U\xaa8-\0\0\0 \0\0\x03\\\0\0\x03\x80\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"\x01\0\0\0\0\0\0\0\0\0\0\x17\x01\0\0\x01\0\0\0\0\0\0\0\x17\x01\0\0\x05\0\0\0\x01",
        b"\0\0\0\x10\0\0\0\n\0\0\x03\x80\0\0\0\x0e\0\0\0+\0\0\x03\xa0\0\0\0\x0e\0\0\0L",
        b"\0\0\x03\xc0\0\0\0\x10\0\0\0o\0\0\x03\xe0\0\0\0\x0f\0\0\0\x91\0\0\x04\0\0\0\0\x11",
        b"\0\0\0\xb2\0\0\x04 \0\0\0\x11\0\0\0\xd3\0\0\x04@\0\0\0\x13\0\0\0\xf6\0\0\x04`",
        b"\0\0\0\x12\0\0\x01\x18\0\0\x04\x80\0\0\0\x0e\0\0\x019\0\0\x04\xa0\0\0\0\x0e\0\0\x01Z",
        b"\0\0\x04\xc0\0\0\0\x10\0\0\x01}\0\0\x04\xe0\0\0\0\x0f\0\0\x01\x9f\0\0\x05\0\0\0\0\x08",
        b"\x01\0\x01\xb7\0\0\0\x01\0\0\0\x14\0\0\x01\xbc\0\0\x05 \0\0\0\x05\0\0\x01\xd5\0\0\x05@",
        b"\0\0\0\x08\0\0\x01\xee\0\0\x05`\0\0\0\x05\x01\0\x02\x07\0\0\0\x01\0\0\0\x17\0\0\x02\x0c",
        b"\0\0\x05\x80\0\0\0\x03\0\0\x02'\0\0\x05\xa0\0\0\0\x03\0arc\0anim\0op",
        b"eningTitle_CN_00_inPress.brlan\0o",
        b"peningTitle_CN_00_inTitle.brlan\0",
        b"openingTitle_CN_00_loopPress.brl",
        b"an\0openingTitle_CN_00_outPress.b",
        b"rlan\0openingTitle_EU_00_inPress.",
        b"brlan\0openingTitle_EU_00_inTitle",
        b".brlan\0openingTitle_EU_00_loopPr",
        b"ess.brlan\0openingTitle_EU_00_out",
        b"Press.brlan\0openingTitle_US_00_i",
        b"nPress.brlan\0openingTitle_US_00_",
        b"inTitle.brlan\0openingTitle_US_00",
        b"_loopPress.brlan\0openingTitle_US",
        b"_00_outPress.brlan\0some other ra",
        b"ndom thing\0blyt\0openingTitle_CN_",
        b"00.brlyt\0openingTitle_EU_00.brly",
        b"t\0openingTitle_US_00.brlyt\0timg\0",
        b"wiiMario_Title_logo_CN.tpl\0wiiMa",
        b"rio_Title_logo_local_00.tpl\0\0\0\0\0",
        b"in_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_title_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"loop_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"out_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_press_brlan_eu\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_title_brlan_eu\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"loop_press_brlan_eu\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"out_press_brlan_eu\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_title_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"loop_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"out_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"whatever\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"brlyt\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"brlyt_eu\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"brlyt\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"tpl\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"tpl",
    ))?;

    let mut cmd = Command::cargo_bin(BIN_NAME)?;
    cmd.args(["--from", "e,c"])
        .args(["--to", "w"])
        .arg(filepath.path());
    cmd.assert().success();

    filepath.assert(concat_bytes!(
        b"U\xaa8-\0\0\0 \0\0\x02\x80\0\0\x02\xa0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"\x01\0\0\0\0\0\0\0\0\0\0\x12\x01\0\0\x01\0\0\0\0\0\0\0\x12\x01\0\0\x05\0\0\0\x01",
        b"\0\0\0\x0c\0\0\0\n\0\0\x02\xa0\0\0\0\x11\0\0\0+\0\0\x02\xc0\0\0\0\x11\0\0\0L",
        b"\0\0\x02\xe0\0\0\0\x13\0\0\0o\0\0\x03\0\0\0\0\x12\0\0\0\x91\0\0\x03 \0\0\0\x0e",
        b"\0\0\0\xb2\0\0\x03@\0\0\0\x0e\0\0\0\xd3\0\0\x03`\0\0\0\x10\0\0\0\xf6\0\0\x03\x80",
        b"\0\0\0\x0f\0\0\x01\x18\0\0\x03\xa0\0\0\0\x08\x01\0\x010\0\0\0\x01\0\0\0\x0f\0\0\x015",
        b"\0\0\x03\xc0\0\0\0\x08\0\0\x01N\0\0\x03\xe0\0\0\0\x05\x01\0\x01g\0\0\0\x01\0\0\0\x12",
        b"\0\0\x01l\0\0\x04\0\0\0\0\x03\0\0\x01\x87\0\0\x04 \0\0\0\x03\0arc\0ani",
        b"m\0openingTitle_EU_00_inPress.brl",
        b"an\0openingTitle_EU_00_inTitle.br",
        b"lan\0openingTitle_EU_00_loopPress",
        b".brlan\0openingTitle_EU_00_outPre",
        b"ss.brlan\0openingTitle_TW_00_inPr",
        b"ess.brlan\0openingTitle_TW_00_inT",
        b"itle.brlan\0openingTitle_TW_00_lo",
        b"opPress.brlan\0openingTitle_TW_00",
        b"_outPress.brlan\0some other rando",
        b"m thing\0blyt\0openingTitle_EU_00.",
        b"brlyt\0openingTitle_TW_00.brlyt\0t",
        b"img\0wiiMario_Title_logo_CN.tpl\0w",
        b"iiMario_Title_logo_local_00.tpl\0",
        b"in_press_brlan_eu\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_title_brlan_eu\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"loop_press_brlan_eu\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"out_press_brlan_eu\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_title_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"loop_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"out_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"whatever\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"brlyt_eu\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"brlyt\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"tpl\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"tpl",
    ) as &[u8]);

    Ok(())
}

#[test]
fn test_arg_to() -> Result<()> {
    let filepath = NamedTempFile::new("test.arc")?;

    // {
    //     "arc": {
    //         "anim": {
    //             "openingTitle_EU_00_inPress.brlan": b"in_press_brlan",
    //             "openingTitle_EU_00_inTitle.brlan": b"in_title_brlan",
    //             "openingTitle_EU_00_loopPress.brlan": b"loop_press_brlan",
    //             "openingTitle_EU_00_outPress.brlan": b"out_press_brlan",
    //             "some other random thing": b"whatever"},
    //         "blyt": {
    //             "openingTitle_EU_00.brlyt": b"brlyt"},
    //         "timg": {
    //             "wiiMario_Title_logo_local_00.tpl": b"tpl"}}}
    filepath.write_binary(concat_bytes!(
        b"U\xaa8-\0\0\0 \0\0\x01}\0\0\x01\xa0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"\x01\0\0\0\0\0\0\0\0\0\0\x0c\x01\0\0\x01\0\0\0\0\0\0\0\x0c\x01\0\0\x05\0\0\0\x01",
        b"\0\0\0\x08\0\0\0\n\0\0\x01\xa0\0\0\0\x0e\0\0\0+\0\0\x01\xc0\0\0\0\x0e\0\0\0L",
        b"\0\0\x01\xe0\0\0\0\x10\0\0\0o\0\0\x02\0\0\0\0\x0f\0\0\0\x91\0\0\x02 \0\0\0\x08",
        b"\x01\0\0\xa9\0\0\0\x01\0\0\0\n\0\0\0\xae\0\0\x02@\0\0\0\x05\x01\0\0\xc7\0\0\0\x01",
        b"\0\0\0\x0c\0\0\0\xcc\0\0\x02`\0\0\0\x03\0arc\0anim\0openin",
        b"gTitle_EU_00_inPress.brlan\0openi",
        b"ngTitle_EU_00_inTitle.brlan\0open",
        b"ingTitle_EU_00_loopPress.brlan\0o",
        b"peningTitle_EU_00_outPress.brlan",
        b"\0some other random thing\0blyt\0op",
        b"eningTitle_EU_00.brlyt\0timg\0wiiM",
        b"ario_Title_logo_local_00.tpl\0\0\0\0",
        b"in_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_title_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"loop_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"out_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"whatever\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"brlyt\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"tpl",
    ))?;

    let mut cmd = Command::cargo_bin(BIN_NAME)?;
    cmd.args(["--to", "j,k"]).arg(filepath.path());
    cmd.assert().success();

    filepath.assert(concat_bytes!(
        b"U\xaa8-\0\0\0 \0\0\x02J\0\0\x02\x80\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"\x01\0\0\0\0\0\0\0\0\0\0\x11\x01\0\0\x01\0\0\0\0\0\0\0\x11\x01\0\0\x05\0\0\0\x01",
        b"\0\0\0\x0c\0\0\0\n\0\0\x02\x80\0\0\0\x0e\0\0\0(\0\0\x02\xa0\0\0\0\x0e\0\0\0F",
        b"\0\0\x02\xc0\0\0\0\x10\0\0\0f\0\0\x02\xe0\0\0\0\x0f\0\0\0\x85\0\0\x02\x80\0\0\0\x0e",
        b"\0\0\0\xa6\0\0\x02\xa0\0\0\0\x0e\0\0\0\xc7\0\0\x02\xc0\0\0\0\x10\0\0\0\xea\0\0\x02\xe0",
        b"\0\0\0\x0f\0\0\x01\x0c\0\0\x03\0\0\0\0\x08\x01\0\x01$\0\0\0\x01\0\0\0\x0f\0\0\x01)",
        b"\0\0\x03 \0\0\0\x05\0\0\x01?\0\0\x03 \0\0\0\x05\x01\0\x01X\0\0\0\x01\0\0\0\x11",
        b"\0\0\x01]\0\0\x03@\0\0\0\x03\0arc\0anim\0openingTit",
        b"le_13_inPress.brlan\0openingTitle",
        b"_13_inTitle.brlan\0openingTitle_1",
        b"3_loopPress.brlan\0openingTitle_1",
        b"3_outPress.brlan\0openingTitle_KR",
        b"_00_inPress.brlan\0openingTitle_K",
        b"R_00_inTitle.brlan\0openingTitle_",
        b"KR_00_loopPress.brlan\0openingTit",
        b"le_KR_00_outPress.brlan\0some oth",
        b"er random thing\0blyt\0openingTitl",
        b"e_13.brlyt\0openingTitle_KR_00.br",
        b"lyt\0timg\0wiiMario_Title_logo_loc",
        b"al_00.tpl\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_title_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"loop_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"out_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"whatever\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"brlyt\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"tpl",
    ) as &[u8]);

    Ok(())
}

#[test]
fn test_arg_output() -> Result<()> {
    let in_filepath = NamedTempFile::new("test_in.arc")?;
    let out_filepath = NamedTempFile::new("test_out.arc")?;

    // {
    //     "arc": {
    //         "anim": {
    //             "openingTitle_TW_00_inPress.brlan": b"in_press_brlan",
    //             "openingTitle_TW_00_inTitle.brlan": b"in_title_brlan",
    //             "openingTitle_TW_00_loopPress.brlan": b"loop_press_brlan",
    //             "openingTitle_TW_00_outPress.brlan": b"out_press_brlan",
    //             "some other random thing": b"whatever"},
    //         "blyt": {
    //             "openingTitle_TW_00.brlyt": b"brlyt"},
    //         "timg": {
    //             "wiiMario_Title_logo_TW.tpl": b"tpl"}}}
    let in_data = concat_bytes!(
        b"U\xaa8-\0\0\0 \0\0\x01w\0\0\x01\xa0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"\x01\0\0\0\0\0\0\0\0\0\0\x0c\x01\0\0\x01\0\0\0\0\0\0\0\x0c\x01\0\0\x05\0\0\0\x01",
        b"\0\0\0\x08\0\0\0\n\0\0\x01\xa0\0\0\0\x0e\0\0\0+\0\0\x01\xc0\0\0\0\x0e\0\0\0L",
        b"\0\0\x01\xe0\0\0\0\x10\0\0\0o\0\0\x02\0\0\0\0\x0f\0\0\0\x91\0\0\x02 \0\0\0\x08",
        b"\x01\0\0\xa9\0\0\0\x01\0\0\0\n\0\0\0\xae\0\0\x02@\0\0\0\x05\x01\0\0\xc7\0\0\0\x01",
        b"\0\0\0\x0c\0\0\0\xcc\0\0\x02`\0\0\0\x03\0arc\0anim\0openin",
        b"gTitle_TW_00_inPress.brlan\0openi",
        b"ngTitle_TW_00_inTitle.brlan\0open",
        b"ingTitle_TW_00_loopPress.brlan\0o",
        b"peningTitle_TW_00_outPress.brlan",
        b"\0some other random thing\0blyt\0op",
        b"eningTitle_TW_00.brlyt\0timg\0wiiM",
        b"ario_Title_logo_TW.tpl\0\0\0\0\0\0\0\0\0\0",
        b"in_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_title_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"loop_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"out_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"whatever\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"brlyt\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"tpl",
    );

    in_filepath.write_binary(in_data as &[u8])?;

    let mut cmd = Command::cargo_bin(BIN_NAME)?;
    cmd.arg("-o")
        .arg(out_filepath.path())
        .arg(in_filepath.path());
    cmd.assert().success();

    in_filepath.assert(in_data as &[u8]);
    out_filepath.assert(concat_bytes!(
        b"U\xaa8-\0\0\0 \0\0\x05\xb4\0\0\x05\xe0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"\x01\0\0\0\0\0\0\0\0\0\0%\x01\0\0\x01\0\0\0\0\0\0\0%\x01\0\0\x05\0\0\0\x01",
        b"\0\0\0\x1c\0\0\0\n\0\0\x05\xe0\0\0\0\x0e\0\0\0(\0\0\x06\0\0\0\0\x0e\0\0\0F",
        b"\0\0\x06 \0\0\0\x10\0\0\0f\0\0\x06@\0\0\0\x0f\0\0\0\x85\0\0\x05\xe0\0\0\0\x0e",
        b"\0\0\0\xa6\0\0\x06\0\0\0\0\x0e\0\0\0\xc7\0\0\x06 \0\0\0\x10\0\0\0\xea\0\0\x06@",
        b"\0\0\0\x0f\0\0\x01\x0c\0\0\x05\xe0\0\0\0\x0e\0\0\x01-\0\0\x06\0\0\0\0\x0e\0\0\x01N",
        b"\0\0\x06 \0\0\0\x10\0\0\x01q\0\0\x06@\0\0\0\x0f\0\0\x01\x93\0\0\x05\xe0\0\0\0\x0e",
        b"\0\0\x01\xb4\0\0\x06\0\0\0\0\x0e\0\0\x01\xd5\0\0\x06 \0\0\0\x10\0\0\x01\xf8\0\0\x06@",
        b"\0\0\0\x0f\0\0\x02\x1a\0\0\x05\xe0\0\0\0\x0e\0\0\x02;\0\0\x06\0\0\0\0\x0e\0\0\x02\\",
        b"\0\0\x06 \0\0\0\x10\0\0\x02\x7f\0\0\x06@\0\0\0\x0f\0\0\x02\xa1\0\0\x05\xe0\0\0\0\x0e",
        b"\0\0\x02\xc2\0\0\x06\0\0\0\0\x0e\0\0\x02\xe3\0\0\x06 \0\0\0\x10\0\0\x03\x06\0\0\x06@",
        b"\0\0\0\x0f\0\0\x03(\0\0\x06`\0\0\0\x08\x01\0\x03@\0\0\0\x01\0\0\0#\0\0\x03E",
        b"\0\0\x06\x80\0\0\0\x05\0\0\x03[\0\0\x06\x80\0\0\0\x05\0\0\x03t\0\0\x06\x80\0\0\0\x05",
        b"\0\0\x03\x8d\0\0\x06\x80\0\0\0\x05\0\0\x03\xa6\0\0\x06\x80\0\0\0\x05\0\0\x03\xbf\0\0\x06\x80",
        b"\0\0\0\x05\x01\0\x03\xd8\0\0\0\x01\0\0\0%\0\0\x03\xdd\0\0\x06\xa0\0\0\0\x03\0arc",
        b"\0anim\0openingTitle_13_inPress.br",
        b"lan\0openingTitle_13_inTitle.brla",
        b"n\0openingTitle_13_loopPress.brla",
        b"n\0openingTitle_13_outPress.brlan",
        b"\0openingTitle_CN_00_inPress.brla",
        b"n\0openingTitle_CN_00_inTitle.brl",
        b"an\0openingTitle_CN_00_loopPress.",
        b"brlan\0openingTitle_CN_00_outPres",
        b"s.brlan\0openingTitle_EU_00_inPre",
        b"ss.brlan\0openingTitle_EU_00_inTi",
        b"tle.brlan\0openingTitle_EU_00_loo",
        b"pPress.brlan\0openingTitle_EU_00_",
        b"outPress.brlan\0openingTitle_KR_0",
        b"0_inPress.brlan\0openingTitle_KR_",
        b"00_inTitle.brlan\0openingTitle_KR",
        b"_00_loopPress.brlan\0openingTitle",
        b"_KR_00_outPress.brlan\0openingTit",
        b"le_TW_00_inPress.brlan\0openingTi",
        b"tle_TW_00_inTitle.brlan\0openingT",
        b"itle_TW_00_loopPress.brlan\0openi",
        b"ngTitle_TW_00_outPress.brlan\0ope",
        b"ningTitle_US_00_inPress.brlan\0op",
        b"eningTitle_US_00_inTitle.brlan\0o",
        b"peningTitle_US_00_loopPress.brla",
        b"n\0openingTitle_US_00_outPress.br",
        b"lan\0some other random thing\0blyt",
        b"\0openingTitle_13.brlyt\0openingTi",
        b"tle_CN_00.brlyt\0openingTitle_EU_",
        b"00.brlyt\0openingTitle_KR_00.brly",
        b"t\0openingTitle_TW_00.brlyt\0openi",
        b"ngTitle_US_00.brlyt\0timg\0wiiMari",
        b"o_Title_logo_TW.tpl\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_title_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"loop_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"out_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"whatever\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"brlyt\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"tpl",
    ) as &[u8]);

    Ok(())
}

#[test]
fn test_arg_ignore_conflicts_data() -> Result<()> {
    let filepath = NamedTempFile::new("test.arc")?;

    // {
    //     "arc": {
    //         "anim": {
    //             "openingTitle_CN_00_inPress.brlan": b"in_press_brlan_cn",
    //             "openingTitle_EU_00_inPress.brlan": b"in_press_brlan_eu",
    //             "openingTitle_CN_00_inTitle.brlan": b"in_title_brlan",
    //             "openingTitle_CN_00_loopPress.brlan": b"loop_press_brlan",
    //             "openingTitle_CN_00_outPress.brlan": b"out_press_brlan",
    //             "some other random thing": b"whatever"},
    //         "blyt": {
    //             "openingTitle_CN_00.brlyt": b"brlyt"},
    //         "timg": {
    //             "wiiMario_Title_logo_CN.tpl": b"tpl"}}}
    filepath.write_binary(concat_bytes!(
        b"U\xaa8-\0\0\0 \0\0\x01\xa4\0\0\x01\xe0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"\x01\0\0\0\0\0\0\0\0\0\0\r\x01\0\0\x01\0\0\0\0\0\0\0\r\x01\0\0\x05\0\0\0\x01",
        b"\0\0\0\t\0\0\0\n\0\0\x01\xe0\0\0\0\x11\0\0\0+\0\0\x02\0\0\0\0\x0e\0\0\0L",
        b"\0\0\x02 \0\0\0\x10\0\0\0o\0\0\x02@\0\0\0\x0f\0\0\0\x91\0\0\x02`\0\0\0\x11",
        b"\0\0\0\xb2\0\0\x02\x80\0\0\0\x08\x01\0\0\xca\0\0\0\x01\0\0\0\x0b\0\0\0\xcf\0\0\x02\xa0",
        b"\0\0\0\x05\x01\0\0\xe8\0\0\0\x01\0\0\0\r\0\0\0\xed\0\0\x02\xc0\0\0\0\x03\0arc",
        b"\0anim\0openingTitle_CN_00_inPress",
        b".brlan\0openingTitle_CN_00_inTitl",
        b"e.brlan\0openingTitle_CN_00_loopP",
        b"ress.brlan\0openingTitle_CN_00_ou",
        b"tPress.brlan\0openingTitle_EU_00_",
        b"inPress.brlan\0some other random ",
        b"thing\0blyt\0openingTitle_CN_00.br",
        b"lyt\0timg\0wiiMario_Title_logo_CN.",
        b"tpl\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_press_brlan_cn\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_title_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"loop_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"out_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_press_brlan_eu\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"whatever\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"brlyt\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"tpl",
    ))?;

    // Without the --ignore-conflicts flag, this fails because
    // openingTitle_CN_00_inPress.brlan and
    // openingTitle_EU_00_inPress.brlan have conflicting data
    let mut cmd = Command::cargo_bin(BIN_NAME)?;
    cmd.args(["--to", "e"]).arg(filepath.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("conflicting"));

    // With --ignore-conflicts, it succeeds and chooses the EU one over
    // the CN one
    let mut cmd = Command::cargo_bin(BIN_NAME)?;
    cmd.args(["--to", "e"])
        .arg("--ignore-conflicts")
        .arg(filepath.path());
    cmd.assert().success();

    filepath.assert(concat_bytes!(
        b"U\xaa8-\0\0\0 \0\0\x01w\0\0\x01\xa0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"\x01\0\0\0\0\0\0\0\0\0\0\x0c\x01\0\0\x01\0\0\0\0\0\0\0\x0c\x01\0\0\x05\0\0\0\x01",
        b"\0\0\0\x08\0\0\0\n\0\0\x01\xa0\0\0\0\x11\0\0\0+\0\0\x01\xc0\0\0\0\x0e\0\0\0L",
        b"\0\0\x01\xe0\0\0\0\x10\0\0\0o\0\0\x02\0\0\0\0\x0f\0\0\0\x91\0\0\x02 \0\0\0\x08",
        b"\x01\0\0\xa9\0\0\0\x01\0\0\0\n\0\0\0\xae\0\0\x02@\0\0\0\x05\x01\0\0\xc7\0\0\0\x01",
        b"\0\0\0\x0c\0\0\0\xcc\0\0\x02`\0\0\0\x03\0arc\0anim\0openin",
        b"gTitle_US_00_inPress.brlan\0openi",
        b"ngTitle_US_00_inTitle.brlan\0open",
        b"ingTitle_US_00_loopPress.brlan\0o",
        b"peningTitle_US_00_outPress.brlan",
        b"\0some other random thing\0blyt\0op",
        b"eningTitle_US_00.brlyt\0timg\0wiiM",
        b"ario_Title_logo_CN.tpl\0\0\0\0\0\0\0\0\0\0",
        b"in_press_brlan_eu\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_title_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"loop_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"out_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"whatever\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"brlyt\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"tpl",
    ) as &[u8]);

    Ok(())
}

#[test]
fn test_arg_ignore_conflicts_filenames() -> Result<()> {
    let filepath = NamedTempFile::new("test.arc")?;

    // {
    //     "arc": {
    //         "anim": {
    //             "openingTitle_CN_00_inPress.brlan": b"in_press_brlan",
    //             "openingTitle_CN_00_inTitle.brlan": b"in_title_brlan",
    //             "openingTitle_CN_00_loopPress.brlan": b"loop_press_brlan_cn",
    //             "openingTitle_US_00_loopPress.brlan": b"loop_press_brlan_us",
    //             "openingTitle_CN_00_outPress.brlan": b"out_press_brlan",
    //             "some other random thing": b"whatever"},
    //         "blyt": {
    //             "openingTitle_CN_00.brlyt": b"brlyt"},
    //         "timg": {
    //             "wiiMario_Title_logo_CN.tpl": b"tpl"}}}
    filepath.write_binary(concat_bytes!(
        b"U\xaa8-\0\0\0 \0\0\x01\xa6\0\0\x01\xe0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"\x01\0\0\0\0\0\0\0\0\0\0\r\x01\0\0\x01\0\0\0\0\0\0\0\r\x01\0\0\x05\0\0\0\x01",
        b"\0\0\0\t\0\0\0\n\0\0\x01\xe0\0\0\0\x0e\0\0\0+\0\0\x02\0\0\0\0\x0e\0\0\0L",
        b"\0\0\x02 \0\0\0\x13\0\0\0o\0\0\x02@\0\0\0\x0f\0\0\0\x91\0\0\x02`\0\0\0\x13",
        b"\0\0\0\xb4\0\0\x02\x80\0\0\0\x08\x01\0\0\xcc\0\0\0\x01\0\0\0\x0b\0\0\0\xd1\0\0\x02\xa0",
        b"\0\0\0\x05\x01\0\0\xea\0\0\0\x01\0\0\0\r\0\0\0\xef\0\0\x02\xc0\0\0\0\x03\0arc",
        b"\0anim\0openingTitle_CN_00_inPress",
        b".brlan\0openingTitle_CN_00_inTitl",
        b"e.brlan\0openingTitle_CN_00_loopP",
        b"ress.brlan\0openingTitle_CN_00_ou",
        b"tPress.brlan\0openingTitle_US_00_",
        b"loopPress.brlan\0some other rando",
        b"m thing\0blyt\0openingTitle_CN_00.",
        b"brlyt\0timg\0wiiMario_Title_logo_C",
        b"N.tpl\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_title_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"loop_press_brlan_cn\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"out_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"loop_press_brlan_us\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"whatever\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"brlyt\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"tpl",
    ))?;

    // Without the --ignore-conflicts flag, this fails because
    // openingTitle_US_00_loopPress.brlan already exists
    let mut cmd = Command::cargo_bin(BIN_NAME)?;
    cmd.args(["--from", "c"])
        .args(["--to", "e"])
        .arg(filepath.path());
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));

    // With --ignore-conflicts, it succeeds and overwrites the US one
    // with the CN one
    let mut cmd = Command::cargo_bin(BIN_NAME)?;
    cmd.args(["--from", "c"])
        .args(["--to", "e"])
        .arg("--ignore-conflicts")
        .arg(filepath.path());
    cmd.assert().success();

    filepath.assert(concat_bytes!(
        b"U\xaa8-\0\0\0 \0\0\x01w\0\0\x01\xa0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"\x01\0\0\0\0\0\0\0\0\0\0\x0c\x01\0\0\x01\0\0\0\0\0\0\0\x0c\x01\0\0\x05\0\0\0\x01",
        b"\0\0\0\x08\0\0\0\n\0\0\x01\xa0\0\0\0\x0e\0\0\0+\0\0\x01\xc0\0\0\0\x0e\0\0\0L",
        b"\0\0\x01\xe0\0\0\0\x13\0\0\0o\0\0\x02\0\0\0\0\x0f\0\0\0\x91\0\0\x02 \0\0\0\x08",
        b"\x01\0\0\xa9\0\0\0\x01\0\0\0\n\0\0\0\xae\0\0\x02@\0\0\0\x05\x01\0\0\xc7\0\0\0\x01",
        b"\0\0\0\x0c\0\0\0\xcc\0\0\x02`\0\0\0\x03\0arc\0anim\0openin",
        b"gTitle_US_00_inPress.brlan\0openi",
        b"ngTitle_US_00_inTitle.brlan\0open",
        b"ingTitle_US_00_loopPress.brlan\0o",
        b"peningTitle_US_00_outPress.brlan",
        b"\0some other random thing\0blyt\0op",
        b"eningTitle_US_00.brlyt\0timg\0wiiM",
        b"ario_Title_logo_CN.tpl\0\0\0\0\0\0\0\0\0\0",
        b"in_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"in_title_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"loop_press_brlan_cn\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"out_press_brlan\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"whatever\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"brlyt\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
        b"tpl",
    ) as &[u8]);

    Ok(())
}
