//! Defines constants specifying the region-dependent filenames used in
//! openingTitle.arc in each region.

use phf::phf_map;

/// Container for all region-specific openingTitle.arc filenames for a
/// particular region.
pub struct OpeningTitleRegionFilenames<'a> {
    pub in_press_brlan: &'a str,
    pub in_title_brlan: &'a str,
    pub loop_press_brlan: &'a str,
    pub out_press_brlan: &'a str,
    pub brlyt: &'a str,
}

/// Filenames for the "P" (international) region.
const P_FILENAMES: OpeningTitleRegionFilenames<'static> = OpeningTitleRegionFilenames {
    in_press_brlan: "openingTitle_EU_00_inPress.brlan",
    in_title_brlan: "openingTitle_EU_00_inTitle.brlan",
    loop_press_brlan: "openingTitle_EU_00_loopPress.brlan",
    out_press_brlan: "openingTitle_EU_00_outPress.brlan",
    brlyt: "openingTitle_EU_00.brlyt",
    // (for reference) TPL: "wiiMario_Title_logo_local_00.tpl"
};

/// Filenames for the "E" (North American) region.
const E_FILENAMES: OpeningTitleRegionFilenames<'static> = OpeningTitleRegionFilenames {
    in_press_brlan: "openingTitle_US_00_inPress.brlan",
    in_title_brlan: "openingTitle_US_00_inTitle.brlan",
    loop_press_brlan: "openingTitle_US_00_loopPress.brlan",
    out_press_brlan: "openingTitle_US_00_outPress.brlan",
    brlyt: "openingTitle_US_00.brlyt",
    // (for reference) TPL: "wiiMario_Title_logo_local_00.tpl"
};

/// Filenames for the "J" (Japanese) region.
const J_FILENAMES: OpeningTitleRegionFilenames<'static> = OpeningTitleRegionFilenames {
    in_press_brlan: "openingTitle_13_inPress.brlan",
    in_title_brlan: "openingTitle_13_inTitle.brlan",
    loop_press_brlan: "openingTitle_13_loopPress.brlan",
    out_press_brlan: "openingTitle_13_outPress.brlan",
    brlyt: "openingTitle_13.brlyt",
    // (for reference) TPL: "wiiMario_Title_logo_00.tpl"
};

/// Filenames for the "K" (Korean) region.
const K_FILENAMES: OpeningTitleRegionFilenames<'static> = OpeningTitleRegionFilenames {
    in_press_brlan: "openingTitle_KR_00_inPress.brlan",
    in_title_brlan: "openingTitle_KR_00_inTitle.brlan",
    loop_press_brlan: "openingTitle_KR_00_loopPress.brlan",
    out_press_brlan: "openingTitle_KR_00_outPress.brlan",
    brlyt: "openingTitle_KR_00.brlyt",
    // (for reference) TPL: "wiiMario_Title_logo_KOR.tpl"
};

/// Filenames for the "W" (Taiwanese) region.
const W_FILENAMES: OpeningTitleRegionFilenames<'static> = OpeningTitleRegionFilenames {
    in_press_brlan: "openingTitle_TW_00_inPress.brlan",
    in_title_brlan: "openingTitle_TW_00_inTitle.brlan",
    loop_press_brlan: "openingTitle_TW_00_loopPress.brlan",
    out_press_brlan: "openingTitle_TW_00_outPress.brlan",
    brlyt: "openingTitle_TW_00.brlyt",
    // (for reference) TPL: "wiiMario_Title_logo_TW.tpl"
};

/// Filenames for the "C" (Chinese) region.
const C_FILENAMES: OpeningTitleRegionFilenames<'static> = OpeningTitleRegionFilenames {
    in_press_brlan: "openingTitle_CN_00_inPress.brlan",
    in_title_brlan: "openingTitle_CN_00_inTitle.brlan",
    loop_press_brlan: "openingTitle_CN_00_loopPress.brlan",
    out_press_brlan: "openingTitle_CN_00_outPress.brlan",
    brlyt: "openingTitle_CN_00.brlyt",
    // (for reference) TPL: "wiiMario_Title_logo_CN.tpl"
};

/// Map that contains the filenames for every region.
pub const ALL_FILENAMES: phf::Map<&'static str, OpeningTitleRegionFilenames> = phf_map! {
    "P" => P_FILENAMES,
    "E" => E_FILENAMES,
    "J" => J_FILENAMES,
    "K" => K_FILENAMES,
    "W" => W_FILENAMES,
    "C" => C_FILENAMES,
};
