// @generated from pret/pokeemerald src/data/credits.h.
// Gen3-encoded and 0xFF-terminated. The page order is `PAGE_*` up to `PAGE_COUNT`.
// Strings the page table does not use stay, so the file still matches `credits.h`.
#![allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub struct Line {
    pub title: bool,
    pub text: &'static [u8],
}

static SCREDITSTEXT_AIMASHIMA: &[u8] = &[
    0xBB, 0xDD, 0x00, 0xC7, 0xD5, 0xE7, 0xDC, 0xDD, 0xE1, 0xD5, 0xFF,
];
static SCREDITSTEXT_AIMITOMITA: &[u8] = &[
    0xBB, 0xDD, 0xE1, 0xDD, 0x00, 0xCE, 0xE3, 0xE1, 0xDD, 0xE8, 0xD5, 0xFF,
];
static SCREDITSTEXT_AKIHITOTOMISAWA: &[u8] = &[
    0xBB, 0xDF, 0xDD, 0xDC, 0xDD, 0xE8, 0xE3, 0x00, 0xCE, 0xE3, 0xE1, 0xDD, 0xE7, 0xD5, 0xEB, 0xD5,
    0xFF,
];
static SCREDITSTEXT_AKIKOSHINOZAKI: &[u8] = &[
    0xBB, 0xDF, 0xDD, 0xDF, 0xE3, 0x00, 0xCD, 0xDC, 0xDD, 0xE2, 0xE3, 0xEE, 0xD5, 0xDF, 0xDD, 0xFF,
];
static SCREDITSTEXT_AKIRAKINASHI: &[u8] = &[
    0xBB, 0xDF, 0xDD, 0xE6, 0xD5, 0x00, 0xC5, 0xDD, 0xE2, 0xD5, 0xE7, 0xDC, 0xDD, 0xFF,
];
static SCREDITSTEXT_AKITOMORI: &[u8] = &[
    0xBB, 0xDF, 0xDD, 0xE8, 0xE3, 0x00, 0xC7, 0xE3, 0xE6, 0xDD, 0xFF,
];
static SCREDITSTEXT_ARTDIRECTOR: &[u8] = &[
    0xBB, 0xE6, 0xE8, 0x00, 0xBE, 0xDD, 0xE6, 0xD9, 0xD7, 0xE8, 0xE3, 0xE6, 0xFF,
];
static SCREDITSTEXT_ARTWORK: &[u8] = &[0xBB, 0xE6, 0xE8, 0xEB, 0xE3, 0xE6, 0xDF, 0xFF];
static SCREDITSTEXT_ASTUKOFUJII: &[u8] = &[
    0xBB, 0xE7, 0xE8, 0xE9, 0xDF, 0xE3, 0x00, 0xC0, 0xE9, 0xDE, 0xDD, 0xDD, 0xFF,
];
static SCREDITSTEXT_ASUKAIWASHITA: &[u8] = &[
    0xBB, 0xE7, 0xE9, 0xDF, 0xD5, 0x00, 0xC3, 0xEB, 0xD5, 0xE7, 0xDC, 0xDD, 0xE8, 0xD5, 0xFF,
];
static SCREDITSTEXT_ATSUKONISHIDA: &[u8] = &[
    0xBB, 0xE8, 0xE7, 0xE9, 0xDF, 0xE3, 0x00, 0xC8, 0xDD, 0xE7, 0xDC, 0xDD, 0xD8, 0xD5, 0xFF,
];
static SCREDITSTEXT_ATSUSHITADA: &[u8] = &[
    0xBB, 0xE8, 0xE7, 0xE9, 0xE7, 0xDC, 0xDD, 0x00, 0xCE, 0xD5, 0xD8, 0xD5, 0xFF,
];
static SCREDITSTEXT_AUSTRALIANBRAILLEAUTHORITY: &[u8] = &[
    0xBB, 0xE9, 0xE7, 0xE8, 0xE6, 0xD5, 0xE0, 0xDD, 0xD5, 0xE2, 0x00, 0xBC, 0xE6, 0xD5, 0xDD, 0xE0,
    0xE0, 0xD9, 0x00, 0xBB, 0xE9, 0xE8, 0xDC, 0xE3, 0xE6, 0xDD, 0xE8, 0xED, 0xFF,
];
static SCREDITSTEXT_AZUSATAJIMA: &[u8] = &[
    0xBB, 0xEE, 0xE9, 0xE7, 0xD5, 0x00, 0xCE, 0xD5, 0xDE, 0xDD, 0xE1, 0xD5, 0xFF,
];
static SCREDITSTEXT_BATTLEDIRECTOR: &[u8] = &[
    0xBC, 0xD5, 0xE8, 0xE8, 0xE0, 0xD9, 0x00, 0xBE, 0xDD, 0xE6, 0xD9, 0xD7, 0xE8, 0xE3, 0xE6, 0xFF,
];
static SCREDITSTEXT_BATTLEFRONTIERDATA: &[u8] = &[
    0xBC, 0xD5, 0xE8, 0xE8, 0xE0, 0xD9, 0x00, 0xC0, 0xE6, 0xE3, 0xE2, 0xE8, 0xDD, 0xD9, 0xE6, 0x00,
    0xBE, 0xD5, 0xE8, 0xD5, 0xFF,
];
static SCREDITSTEXT_BATTLESYSTEMPGRMS: &[u8] = &[
    0xBC, 0xD5, 0xE8, 0xE8, 0xE0, 0xD9, 0x00, 0xCD, 0xED, 0xE7, 0xE8, 0xD9, 0xE1, 0x00, 0xCA, 0xE6,
    0xE3, 0xDB, 0xE6, 0xD5, 0xE1, 0xE1, 0xD9, 0xE6, 0xE7, 0xFF,
];
static SCREDITSTEXT_BRAILLECODECHECK: &[u8] = &[
    0xBC, 0xE6, 0xD5, 0xDD, 0xE0, 0xE0, 0xD9, 0x00, 0xBD, 0xE3, 0xD8, 0xD9, 0x00, 0xBD, 0xDC, 0xD9,
    0xD7, 0xDF, 0xFF,
];
static SCREDITSTEXT_CHIAKISHINKAI: &[u8] = &[
    0xBD, 0xDC, 0xDD, 0xD5, 0xDF, 0xDD, 0x00, 0xCD, 0xDC, 0xDD, 0xE2, 0xDF, 0xD5, 0xDD, 0xFF,
];
static SCREDITSTEXT_CHIEMATSUMIYA: &[u8] = &[
    0xBD, 0xDC, 0xDD, 0xD9, 0x00, 0xC7, 0xD5, 0xE8, 0xE7, 0xE9, 0xE1, 0xDD, 0xED, 0xD5, 0xFF,
];
static SCREDITSTEXT_COORDINATORS: &[u8] = &[
    0xBD, 0xE3, 0xE3, 0xE6, 0xD8, 0xDD, 0xE2, 0xD5, 0xE8, 0xE3, 0xE6, 0xE7, 0xFF,
];
static SCREDITSTEXT_CREDITS: &[u8] = &[0xBD, 0xE6, 0xD9, 0xD8, 0xDD, 0xE8, 0xE7, 0xFF];
static SCREDITSTEXT_DAISUKEHOSHINO: &[u8] = &[
    0xBE, 0xD5, 0xDD, 0xE7, 0xE9, 0xDF, 0xD9, 0x00, 0xC2, 0xE3, 0xE7, 0xDC, 0xDD, 0xE2, 0xE3, 0xFF,
];
static SCREDITSTEXT_DIRECTOR: &[u8] = &[0xBE, 0xDD, 0xE6, 0xD9, 0xD7, 0xE8, 0xE3, 0xE6, 0xFF];
static SCREDITSTEXT_EMPTYSTRING: &[u8] = &[0xFF];
static SCREDITSTEXT_ENGLISHVERSION: &[u8] = &[
    0xBF, 0xE2, 0xDB, 0xE0, 0xDD, 0xE7, 0xDC, 0x00, 0xD0, 0xD9, 0xE6, 0xE7, 0xDD, 0xE3, 0xE2, 0x00,
    0xBD, 0xE3, 0xE3, 0xE6, 0xD8, 0xDD, 0xE2, 0xD5, 0xE8, 0xE3, 0xE6, 0xE7, 0xFF,
];
static SCREDITSTEXT_ENVANDTOOLPGRMS: &[u8] = &[
    0xBF, 0xE2, 0xEA, 0xDD, 0xE6, 0xE3, 0xE2, 0xE1, 0xD9, 0xE2, 0xE8, 0x00, 0x2D, 0x00, 0xCE, 0xE3,
    0xE3, 0xE0, 0x00, 0xCA, 0xE6, 0xE3, 0xDB, 0xE6, 0xD5, 0xE1, 0xE1, 0xD9, 0xE6, 0xE7, 0xFF,
];
static SCREDITSTEXT_ERIKJOHNSON: &[u8] = &[
    0xBF, 0xE6, 0xDD, 0xDF, 0x00, 0xC4, 0xE3, 0xDC, 0xE2, 0xE7, 0xE3, 0xE2, 0xFF,
];
static SCREDITSTEXT_EUROPEANBLINDUNION: &[u8] = &[
    0xBF, 0xE9, 0xE6, 0xE3, 0xE4, 0xD9, 0xD5, 0xE2, 0x00, 0xBC, 0xE0, 0xDD, 0xE2, 0xD8, 0x00, 0xCF,
    0xE2, 0xDD, 0xE3, 0xE2, 0xFF,
];
static SCREDITSTEXT_EXECPRODUCERS: &[u8] = &[
    0xBF, 0xEC, 0xD9, 0xD7, 0xE9, 0xE8, 0xDD, 0xEA, 0xD9, 0x00, 0xCA, 0xE6, 0xE3, 0xD8, 0xE9, 0xD7,
    0xD9, 0xE6, 0xE7, 0xFF,
];
static SCREDITSTEXT_EXECUTIVEDIRECTOR: &[u8] = &[
    0xBF, 0xEC, 0xD9, 0xD7, 0xE9, 0xE8, 0xDD, 0xEA, 0xD9, 0x00, 0xBE, 0xDD, 0xE6, 0xD9, 0xD7, 0xE8,
    0xE3, 0xE6, 0xFF,
];
static SCREDITSTEXT_FIELDSYSTEMPGRMS: &[u8] = &[
    0xC0, 0xDD, 0xD9, 0xE0, 0xD8, 0x00, 0xCD, 0xED, 0xE7, 0xE8, 0xD9, 0xE1, 0x00, 0xCA, 0xE6, 0xE3,
    0xDB, 0xE6, 0xD5, 0xE1, 0xE1, 0xD9, 0xE6, 0xFF,
];
static SCREDITSTEXT_FUJIKONOMURA: &[u8] = &[
    0xC0, 0xE9, 0xDE, 0xDD, 0xDF, 0xE3, 0x00, 0xC8, 0xE3, 0xE1, 0xE9, 0xE6, 0xD5, 0xFF,
];
static SCREDITSTEXT_GAKUJINOMOTO: &[u8] = &[
    0xC1, 0xD5, 0xDF, 0xE9, 0xDE, 0xDD, 0x00, 0xC8, 0xE3, 0xE1, 0xE3, 0xE8, 0xE3, 0xFF,
];
static SCREDITSTEXT_GAMEDESIGNERS: &[u8] = &[
    0xC1, 0xD5, 0xE1, 0xD9, 0x00, 0xBE, 0xD9, 0xE7, 0xDD, 0xDB, 0xE2, 0xD9, 0xE6, 0xE7, 0xFF,
];
static SCREDITSTEXT_GOICHINOSE: &[u8] = &[
    0xC1, 0xE3, 0x00, 0xC3, 0xD7, 0xDC, 0xDD, 0xE2, 0xE3, 0xE7, 0xD9, 0xFF,
];
static SCREDITSTEXT_GRAPHICDESIGNER: &[u8] = &[
    0xC1, 0xE6, 0xD5, 0xE4, 0xDC, 0xDD, 0xD7, 0x00, 0xBE, 0xD9, 0xE7, 0xDD, 0xDB, 0xE2, 0xD9, 0xE6,
    0xFF,
];
static SCREDITSTEXT_GRAPHICDESIGNERS: &[u8] = &[
    0xC1, 0xE6, 0xD5, 0xE4, 0xDC, 0xDD, 0xD7, 0x00, 0xBE, 0xD9, 0xE7, 0xDD, 0xDB, 0xE2, 0xD9, 0xE6,
    0xE7, 0xFF,
];
static SCREDITSTEXT_HIDENORISAEKI: &[u8] = &[
    0xC2, 0xDD, 0xD8, 0xD9, 0xE2, 0xE3, 0xE6, 0xDD, 0x00, 0xCD, 0xD5, 0xD9, 0xDF, 0xDD, 0xFF,
];
static SCREDITSTEXT_HIDEYUKINAKAJIMA: &[u8] = &[
    0xC2, 0xDD, 0xD8, 0xD9, 0xED, 0xE9, 0xDF, 0xDD, 0x00, 0xC8, 0xD5, 0xDF, 0xD5, 0xDE, 0xDD, 0xE1,
    0xD5, 0xFF,
];
static SCREDITSTEXT_HIRONAKAMURA: &[u8] = &[
    0xC2, 0xDD, 0xE6, 0xE3, 0x00, 0xC8, 0xD5, 0xDF, 0xD5, 0xE1, 0xE9, 0xE6, 0xD5, 0xFF,
];
static SCREDITSTEXT_HIROAKITSURU: &[u8] = &[
    0xC2, 0xDD, 0xE6, 0xE3, 0xD5, 0xDF, 0xDD, 0x00, 0xCE, 0xE7, 0xE9, 0xE6, 0xE9, 0xFF,
];
static SCREDITSTEXT_HIROKIENOMOTO: &[u8] = &[
    0xC2, 0xDD, 0xE6, 0xE3, 0xDF, 0xDD, 0x00, 0xBF, 0xE2, 0xE3, 0xE1, 0xE3, 0xE8, 0xE3, 0xFF,
];
static SCREDITSTEXT_HIROMISAGAWA: &[u8] = &[
    0xC2, 0xDD, 0xE6, 0xE3, 0xE1, 0xDD, 0x00, 0xCD, 0xD5, 0xDB, 0xD5, 0xEB, 0xD5, 0xFF,
];
static SCREDITSTEXT_HIRONOBUYOSHIDA: &[u8] = &[
    0xC2, 0xDD, 0xE6, 0xE3, 0xE2, 0xE3, 0xD6, 0xE9, 0x00, 0xD3, 0xE3, 0xE7, 0xDC, 0xDD, 0xD8, 0xD5,
    0xFF,
];
static SCREDITSTEXT_HIROYUKIJINNAI: &[u8] = &[
    0xC2, 0xDD, 0xE6, 0xE3, 0xED, 0xE9, 0xDF, 0xDD, 0x00, 0xC4, 0xDD, 0xE2, 0xE2, 0xD5, 0xDD, 0xFF,
];
static SCREDITSTEXT_HIROYUKINAKAMURA: &[u8] = &[
    0xC2, 0xDD, 0xE6, 0xE3, 0xED, 0xE9, 0xDF, 0xDD, 0x00, 0xC8, 0xD5, 0xDF, 0xD5, 0xE1, 0xE9, 0xE6,
    0xD5, 0xFF,
];
static SCREDITSTEXT_HIROYUKIUESUGI: &[u8] = &[
    0xC2, 0xDD, 0xE6, 0xE3, 0xED, 0xE9, 0xDF, 0xDD, 0x00, 0xCF, 0xD9, 0xE7, 0xE9, 0xDB, 0xDD, 0xFF,
];
static SCREDITSTEXT_HISASHISOGABE: &[u8] = &[
    0xC2, 0xDD, 0xE7, 0xD5, 0xE7, 0xDC, 0xDD, 0x00, 0xCD, 0xE3, 0xDB, 0xD5, 0xD6, 0xD9, 0xFF,
];
static SCREDITSTEXT_HITOMISATO: &[u8] = &[
    0xC2, 0xDD, 0xE8, 0xE3, 0xE1, 0xDD, 0x00, 0xCD, 0xD5, 0xE8, 0xE3, 0xFF,
];
static SCREDITSTEXT_HITOSHIYAMAGAMI: &[u8] = &[
    0xC2, 0xDD, 0xE8, 0xE3, 0xE7, 0xDC, 0xDD, 0x00, 0xD3, 0xD5, 0xE1, 0xD5, 0xDB, 0xD5, 0xE1, 0xDD,
    0xFF,
];
static SCREDITSTEXT_INFOSUPERVISORS: &[u8] = &[
    0xC3, 0xE2, 0xDA, 0xE3, 0xE6, 0xE1, 0xD5, 0xE8, 0xDD, 0xE3, 0xE2, 0x00, 0xCD, 0xE9, 0xE4, 0xD9,
    0xE6, 0xEA, 0xDD, 0xE7, 0xE3, 0xE6, 0xE7, 0xFF,
];
static SCREDITSTEXT_JAPANBRAILLELIBRARY: &[u8] = &[
    0xC4, 0xD5, 0xE4, 0xD5, 0xE2, 0x00, 0xBC, 0xE6, 0xD5, 0xDD, 0xE0, 0xE0, 0xD9, 0x00, 0xC6, 0xDD,
    0xD6, 0xE6, 0xD5, 0xE6, 0xED, 0xFF,
];
static SCREDITSTEXT_JUNOKUTANI: &[u8] = &[
    0xC4, 0xE9, 0xE2, 0x00, 0xC9, 0xDF, 0xE9, 0xE8, 0xD5, 0xE2, 0xDD, 0xFF,
];
static SCREDITSTEXT_JUNICHIMASUDA: &[u8] = &[
    0xC4, 0xE9, 0xE2, 0xDD, 0xD7, 0xDC, 0xDD, 0x00, 0xC7, 0xD5, 0xE7, 0xE9, 0xD8, 0xD5, 0xFF,
];
static SCREDITSTEXT_KANAKOEO: &[u8] = &[0xC5, 0xD5, 0xE2, 0xD5, 0xDF, 0xE3, 0x00, 0xBF, 0xE3, 0xFF];
static SCREDITSTEXT_KAZUKIYOSHIHARA: &[u8] = &[
    0xC5, 0xD5, 0xEE, 0xE9, 0xDF, 0xDD, 0x00, 0xD3, 0xE3, 0xE7, 0xDC, 0xDD, 0xDC, 0xD5, 0xE6, 0xD5,
    0xFF,
];
static SCREDITSTEXT_KAZUYASUYAMA: &[u8] = &[
    0xC5, 0xD5, 0xEE, 0xE9, 0xED, 0xD5, 0x00, 0xCD, 0xE9, 0xED, 0xD5, 0xE1, 0xD5, 0xFF,
];
static SCREDITSTEXT_KAZUYUKITERADA: &[u8] = &[
    0xC5, 0xD5, 0xEE, 0xE9, 0xED, 0xE9, 0xDF, 0xDD, 0x00, 0xCE, 0xD9, 0xE6, 0xD5, 0xD8, 0xD5, 0xFF,
];
static SCREDITSTEXT_KEITAKAGAYA: &[u8] = &[
    0xC5, 0xD9, 0xDD, 0xE8, 0xD5, 0x00, 0xC5, 0xD5, 0xDB, 0xD5, 0xED, 0xD5, 0xFF,
];
static SCREDITSTEXT_KENSUGIMORI: &[u8] = &[
    0xC5, 0xD9, 0xE2, 0x00, 0xCD, 0xE9, 0xDB, 0xDD, 0xE1, 0xE3, 0xE6, 0xDD, 0xFF,
];
static SCREDITSTEXT_KENJIMATSUSHIMA: &[u8] = &[
    0xC5, 0xD9, 0xE2, 0xDE, 0xDD, 0x00, 0xC7, 0xD5, 0xE8, 0xE7, 0xE9, 0xE7, 0xDC, 0xDD, 0xE1, 0xD5,
    0xFF,
];
static SCREDITSTEXT_KENJITOMINAGA: &[u8] = &[
    0xC5, 0xD9, 0xE2, 0xDE, 0xDD, 0x00, 0xCE, 0xE3, 0xE1, 0xDD, 0xE2, 0xD5, 0xDB, 0xD5, 0xFF,
];
static SCREDITSTEXT_KENJIROITO: &[u8] = &[
    0xC5, 0xD9, 0xE2, 0xDE, 0xDD, 0xE6, 0xE3, 0x00, 0xC3, 0xE8, 0xE3, 0xFF,
];
static SCREDITSTEXT_KENKICHITOYAMA: &[u8] = &[
    0xC5, 0xD9, 0xE2, 0xDF, 0xDD, 0xD7, 0xDC, 0xDD, 0x00, 0xCE, 0xE3, 0xED, 0xD5, 0xE1, 0xD5, 0xFF,
];
static SCREDITSTEXT_KIMIKONAKAMICHI: &[u8] = &[
    0xC5, 0xDD, 0xE1, 0xDD, 0xDF, 0xE3, 0x00, 0xC8, 0xD5, 0xDF, 0xD5, 0xE1, 0xDD, 0xD7, 0xDC, 0xDD,
    0xFF,
];
static SCREDITSTEXT_KOJINISHINO: &[u8] = &[
    0xC5, 0xE3, 0xDE, 0xDD, 0x00, 0xC8, 0xDD, 0xE7, 0xDC, 0xDD, 0xE2, 0xE3, 0xFF,
];
static SCREDITSTEXT_KYOKOWATANABE: &[u8] = &[
    0xC5, 0xED, 0xE3, 0xDF, 0xE3, 0x00, 0xD1, 0xD5, 0xE8, 0xD5, 0xE2, 0xD5, 0xD6, 0xD9, 0xFF,
];
static SCREDITSTEXT_LEADGRAPHICARTIST: &[u8] = &[
    0xC6, 0xD9, 0xD5, 0xD8, 0x00, 0xC1, 0xE6, 0xD5, 0xE4, 0xDC, 0xDD, 0xD7, 0x00, 0xBB, 0xE6, 0xE8,
    0xDD, 0xE7, 0xE8, 0xFF,
];
static SCREDITSTEXT_LEADPROGRAMMER: &[u8] = &[
    0xC6, 0xD9, 0xD5, 0xD8, 0x00, 0xCA, 0xE6, 0xE3, 0xDB, 0xE6, 0xD5, 0xE1, 0xE1, 0xD9, 0xE6, 0xFF,
];
static SCREDITSTEXT_MAINGRAPHICDESIGNER: &[u8] = &[
    0xC7, 0xD5, 0xDD, 0xE2, 0x00, 0xC1, 0xE6, 0xD5, 0xE4, 0xDC, 0xDD, 0xD7, 0x00, 0xBE, 0xD9, 0xE7,
    0xDD, 0xDB, 0xE2, 0xD9, 0xE6, 0xFF,
];
static SCREDITSTEXT_MAINPROGRAMMER: &[u8] = &[
    0xC7, 0xD5, 0xDD, 0xE2, 0x00, 0xCA, 0xE6, 0xE3, 0xDB, 0xE6, 0xD5, 0xE1, 0xE1, 0xD9, 0xE6, 0xFF,
];
static SCREDITSTEXT_MAKIKOTAKADA: &[u8] = &[
    0xC7, 0xD5, 0xDF, 0xDD, 0xDF, 0xE3, 0x00, 0xCE, 0xD5, 0xDF, 0xD5, 0xD8, 0xD5, 0xFF,
];
static SCREDITSTEXT_MAPDATADESIGNERS: &[u8] = &[
    0xC7, 0xD5, 0xE4, 0x00, 0xBE, 0xD5, 0xE8, 0xD5, 0x00, 0xBE, 0xD9, 0xE7, 0xDD, 0xDB, 0xE2, 0xD9,
    0xE6, 0xE7, 0xFF,
];
static SCREDITSTEXT_MAPDESIGNERS: &[u8] = &[
    0xC7, 0xD5, 0xE4, 0x00, 0xBE, 0xD9, 0xE7, 0xDD, 0xDB, 0xE2, 0xD9, 0xE6, 0xE7, 0xFF,
];
static SCREDITSTEXT_MASAOTAYA: &[u8] = &[
    0xC7, 0xD5, 0xE7, 0xD5, 0xE3, 0x00, 0xCE, 0xD5, 0xED, 0xD5, 0xFF,
];
static SCREDITSTEXT_MICHIKOTAKIZAWA: &[u8] = &[
    0xC7, 0xDD, 0xD7, 0xDC, 0xDD, 0xDF, 0xE3, 0x00, 0xCE, 0xD5, 0xDF, 0xDD, 0xEE, 0xD5, 0xEB, 0xD5,
    0xFF,
];
static SCREDITSTEXT_MIKAKUROSAWA: &[u8] = &[
    0xC7, 0xDD, 0xDF, 0xD5, 0x00, 0xC5, 0xE9, 0xE6, 0xE3, 0xE7, 0xD5, 0xEB, 0xD5, 0xFF,
];
static SCREDITSTEXT_MIKIKOOHHASHI: &[u8] = &[
    0xC7, 0xDD, 0xDF, 0xDD, 0xDF, 0xE3, 0x00, 0xC9, 0xDC, 0xDC, 0xD5, 0xE7, 0xDC, 0xDD, 0xFF,
];
static SCREDITSTEXT_MIYUKIIWASAWA: &[u8] = &[
    0xC7, 0xDD, 0xED, 0xE9, 0xDF, 0xDD, 0x00, 0xC3, 0xEB, 0xD5, 0xE7, 0xD5, 0xEB, 0xD5, 0xFF,
];
static SCREDITSTEXT_MORIKAZUAOKI: &[u8] = &[
    0xC7, 0xE3, 0xE6, 0xDD, 0xDF, 0xD5, 0xEE, 0xE9, 0x00, 0xBB, 0xE3, 0xDF, 0xDD, 0xFF,
];
static SCREDITSTEXT_MOTOFUMIFUJIWARA: &[u8] = &[
    0xC7, 0xE3, 0xE8, 0xE3, 0xDA, 0xE9, 0xE1, 0xDD, 0x00, 0xC0, 0xE9, 0xDE, 0xDD, 0xEB, 0xD5, 0xE6,
    0xD5, 0xFF,
];
static SCREDITSTEXT_MOTOYASUTOJIMA: &[u8] = &[
    0xC7, 0xE3, 0xE8, 0xE3, 0xED, 0xD5, 0xE7, 0xE9, 0x00, 0xCE, 0xE3, 0xDE, 0xDD, 0xE1, 0xD5, 0xFF,
];
static SCREDITSTEXT_MUNEOSAITO: &[u8] = &[
    0xC7, 0xE9, 0xE2, 0xD9, 0xE3, 0x00, 0xCD, 0xD5, 0xDD, 0xE8, 0xE3, 0xFF,
];
static SCREDITSTEXT_MUSICCOMPOSITION: &[u8] = &[
    0xC7, 0xE9, 0xE7, 0xDD, 0xD7, 0x00, 0xBD, 0xE3, 0xE1, 0xE4, 0xE3, 0xE7, 0xDD, 0xE8, 0xDD, 0xE3,
    0xE2, 0xFF,
];
static SCREDITSTEXT_NCLCOORDINATOR: &[u8] = &[
    0xC8, 0xBD, 0xC6, 0x00, 0xBD, 0xE3, 0xE3, 0xE6, 0xD8, 0xDD, 0xE2, 0xD5, 0xE8, 0xE3, 0xE6, 0xFF,
];
static SCREDITSTEXT_NCLPRODUCTTESTING: &[u8] = &[
    0xC8, 0xBD, 0xC6, 0x00, 0xCA, 0xE6, 0xE3, 0xD8, 0xE9, 0xD7, 0xE8, 0x00, 0xCE, 0xD9, 0xE7, 0xE8,
    0xDD, 0xE2, 0xDB, 0xFF,
];
static SCREDITSTEXT_NCLSUPERMARIOCLUB: &[u8] = &[
    0xC8, 0xBD, 0xC6, 0x00, 0xCD, 0xE9, 0xE4, 0xD9, 0xE6, 0x00, 0xC7, 0xD5, 0xE6, 0xDD, 0xE3, 0x00,
    0xBD, 0xE0, 0xE9, 0xD6, 0xFF,
];
static SCREDITSTEXT_NOAPRODUCTTESTING: &[u8] = &[
    0xC8, 0xC9, 0xBB, 0x00, 0xCA, 0xE6, 0xE3, 0xD8, 0xE9, 0xD7, 0xE8, 0x00, 0xCE, 0xD9, 0xE7, 0xE8,
    0xDD, 0xE2, 0xDB, 0xFF,
];
static SCREDITSTEXT_NAOKOYANASE: &[u8] = &[
    0xC8, 0xD5, 0xE3, 0xDF, 0xE3, 0x00, 0xD3, 0xD5, 0xE2, 0xD5, 0xE7, 0xD9, 0xFF,
];
static SCREDITSTEXT_NATIONALFEDERATIONBLIND: &[u8] = &[
    0xC8, 0xD5, 0xE8, 0xDD, 0xE3, 0xE2, 0xD5, 0xE0, 0x00, 0xC0, 0xD9, 0xD8, 0xD9, 0xE6, 0xD5, 0xE8,
    0xDD, 0xE3, 0xE2, 0x00, 0xE3, 0xDA, 0x00, 0xE8, 0xDC, 0xD9, 0x00, 0xBC, 0xE0, 0xDD, 0xE2, 0xD8,
    0xFF,
];
static SCREDITSTEXT_NICOLAPRATTBARLOW: &[u8] = &[
    0xC8, 0xDD, 0xD7, 0xE3, 0xE0, 0xD5, 0x00, 0xCA, 0xE6, 0xD5, 0xE8, 0xE8, 0xAE, 0xBC, 0xD5, 0xE6,
    0xE0, 0xE3, 0xEB, 0xFF,
];
static SCREDITSTEXT_NOBOGASAWARA: &[u8] = &[
    0xC8, 0xE3, 0xD6, 0x00, 0xC9, 0xDB, 0xD5, 0xE7, 0xD5, 0xEB, 0xD5, 0xE6, 0xD5, 0xFF,
];
static SCREDITSTEXT_NORIHIDEOKAMURA: &[u8] = &[
    0xC8, 0xE3, 0xE6, 0xDD, 0xDC, 0xDD, 0xD8, 0xD9, 0x00, 0xC9, 0xDF, 0xD5, 0xE1, 0xE9, 0xE6, 0xD5,
    0xFF,
];
static SCREDITSTEXT_NOZOMUSAITO: &[u8] = &[
    0xC8, 0xE3, 0xEE, 0xE3, 0xE1, 0xE9, 0x00, 0xCD, 0xD5, 0xDD, 0xE8, 0xE3, 0xFF,
];
static SCREDITSTEXT_PACKAGEANDMANUAL: &[u8] = &[
    0xCA, 0xD5, 0xD7, 0xDF, 0xD5, 0xDB, 0xD9, 0x00, 0x2D, 0x00, 0xC7, 0xD5, 0xE2, 0xE9, 0xD5, 0xE0,
    0x00, 0xC3, 0xE0, 0xE0, 0xE9, 0xE7, 0xE8, 0xE6, 0xD5, 0xE8, 0xDD, 0xE3, 0xE2, 0xFF,
];
static SCREDITSTEXT_PARAMETRICDESIGNERS: &[u8] = &[
    0xCA, 0xD5, 0xE6, 0xD5, 0xE1, 0xD9, 0xE8, 0xE6, 0xDD, 0xD7, 0x00, 0xBE, 0xD9, 0xE7, 0xDD, 0xDB,
    0xE2, 0xD9, 0xE6, 0xE7, 0xFF,
];
static SCREDITSTEXT_PATRICIAAMAURER: &[u8] = &[
    0xCA, 0xD5, 0xE8, 0xE6, 0xDD, 0xD7, 0xDD, 0xD5, 0x00, 0xBB, 0xAD, 0x00, 0xC7, 0xD5, 0xE9, 0xE6,
    0xD9, 0xE6, 0xFF,
];
static SCREDITSTEXT_PKMNDESIGNERS: &[u8] = &[
    0xCA, 0xC9, 0xC5, 0x1B, 0xC7, 0xC9, 0xC8, 0x00, 0xBE, 0xD9, 0xE7, 0xDD, 0xDB, 0xE2, 0xD9, 0xE6,
    0xE7, 0xFF,
];
static SCREDITSTEXT_PKMNEMERALDVERSION: &[u8] = &[
    0xCA, 0xC9, 0xC5, 0x1B, 0xC7, 0xC9, 0xC8, 0x00, 0xBF, 0xC7, 0xBF, 0xCC, 0xBB, 0xC6, 0xBE, 0x00,
    0xD0, 0xBF, 0xCC, 0xCD, 0xC3, 0xC9, 0xC8, 0xFF,
];
static SCREDITSTEXT_POKEDEXTEXT: &[u8] = &[
    0xCA, 0xC9, 0xC5, 0x1B, 0xBE, 0xBF, 0xD2, 0x00, 0xCE, 0xD9, 0xEC, 0xE8, 0xFF,
];
static SCREDITSTEXT_PRODUCERS: &[u8] =
    &[0xCA, 0xE6, 0xE3, 0xD8, 0xE9, 0xD7, 0xD9, 0xE6, 0xE7, 0xFF];
static SCREDITSTEXT_PROGRAMMERS: &[u8] = &[
    0xCA, 0xE6, 0xE3, 0xDB, 0xE6, 0xD5, 0xE1, 0xE1, 0xD9, 0xE6, 0xE7, 0xFF,
];
static SCREDITSTEXT_RENAYOSHIKAWA: &[u8] = &[
    0xCC, 0xD9, 0xE2, 0xD5, 0x00, 0xD3, 0xE3, 0xE7, 0xDC, 0xDD, 0xDF, 0xD5, 0xEB, 0xD5, 0xFF,
];
static SCREDITSTEXT_RETSUJINOMOTO: &[u8] = &[
    0xCC, 0xD9, 0xE8, 0xE7, 0xE9, 0xDE, 0xDD, 0x00, 0xC8, 0xE3, 0xE1, 0xE3, 0xE8, 0xE3, 0xFF,
];
static SCREDITSTEXT_ROYALNEWZEALANDFEDERATIONBLIND: &[u8] = &[
    0xCC, 0xE3, 0xED, 0xD5, 0xE0, 0x00, 0xC8, 0xD9, 0xEB, 0x00, 0xD4, 0xD9, 0xD5, 0xE0, 0xD5, 0xE2,
    0xD8, 0x00, 0xC0, 0xD9, 0xD8, 0xD9, 0xE6, 0xD5, 0xE8, 0xDD, 0xE3, 0xE2, 0x00, 0xDA, 0xE3, 0xE6,
    0x00, 0xE8, 0xDC, 0xD9, 0x00, 0xBC, 0xE0, 0xDD, 0xE2, 0xD8, 0xFF,
];
static SCREDITSTEXT_RUIKAWAGUCHI: &[u8] = &[
    0xCC, 0xE9, 0xDD, 0x00, 0xC5, 0xD5, 0xEB, 0xD5, 0xDB, 0xE9, 0xD7, 0xDC, 0xDD, 0xFF,
];
static SCREDITSTEXT_SACHIKOHAMANO: &[u8] = &[
    0xCD, 0xD5, 0xD7, 0xDC, 0xDD, 0xDF, 0xE3, 0x00, 0xC2, 0xD5, 0xE1, 0xD5, 0xE2, 0xE3, 0xFF,
];
static SCREDITSTEXT_SACHIKONAKAMICHI: &[u8] = &[
    0xCD, 0xD5, 0xD7, 0xDC, 0xDD, 0xDF, 0xE3, 0x00, 0xC8, 0xD5, 0xDF, 0xD5, 0xE1, 0xDD, 0xD7, 0xDC,
    0xDD, 0xFF,
];
static SCREDITSTEXT_SAKAEKIMURA: &[u8] = &[
    0xCD, 0xD5, 0xDF, 0xD5, 0xD9, 0x00, 0xC5, 0xDD, 0xE1, 0xE9, 0xE6, 0xD5, 0xFF,
];
static SCREDITSTEXT_SATORUIWATA: &[u8] = &[
    0xCD, 0xD5, 0xE8, 0xE3, 0xE6, 0xE9, 0x00, 0xC3, 0xEB, 0xD5, 0xE8, 0xD5, 0xFF,
];
static SCREDITSTEXT_SATOSHIMITSUHARA: &[u8] = &[
    0xCD, 0xD5, 0xE8, 0xE3, 0xE7, 0xDC, 0xDD, 0x00, 0xC7, 0xDD, 0xE8, 0xE7, 0xE9, 0xDC, 0xD5, 0xE6,
    0xD5, 0xFF,
];
static SCREDITSTEXT_SATOSHINOHARA: &[u8] = &[
    0xCD, 0xD5, 0xE8, 0xE3, 0xE7, 0xDC, 0xDD, 0x00, 0xC8, 0xE3, 0xDC, 0xD5, 0xE6, 0xD5, 0xFF,
];
static SCREDITSTEXT_SATOSHIOHTA: &[u8] = &[
    0xCD, 0xD5, 0xE8, 0xE3, 0xE7, 0xDC, 0xDD, 0x00, 0xC9, 0xDC, 0xE8, 0xD5, 0xFF,
];
static SCREDITSTEXT_SATOSHITAJIRI: &[u8] = &[
    0xCD, 0xD5, 0xE8, 0xE3, 0xE7, 0xDC, 0xDD, 0x00, 0xCE, 0xD5, 0xDE, 0xDD, 0xE6, 0xDD, 0xFF,
];
static SCREDITSTEXT_SCENARIO: &[u8] = &[0xCD, 0xD7, 0xD9, 0xE2, 0xD5, 0xE6, 0xDD, 0xE3, 0xFF];
static SCREDITSTEXT_SCENARIOPLOT: &[u8] = &[
    0xCD, 0xD7, 0xD9, 0xE2, 0xD5, 0xE6, 0xDD, 0xE3, 0x00, 0xCA, 0xE0, 0xE3, 0xE8, 0xFF,
];
static SCREDITSTEXT_SCRIPTDESIGNERS: &[u8] = &[
    0xCD, 0xD7, 0xE6, 0xDD, 0xE4, 0xE8, 0x00, 0xBE, 0xD9, 0xE7, 0xDD, 0xDB, 0xE2, 0xD9, 0xE6, 0xE7,
    0xFF,
];
static SCREDITSTEXT_SETHMCMAHILL: &[u8] = &[
    0xCD, 0xD9, 0xE8, 0xDC, 0x00, 0xC7, 0xD7, 0xC7, 0xD5, 0xDC, 0xDD, 0xE0, 0xE0, 0xFF,
];
static SCREDITSTEXT_SHELLIEDOW: &[u8] = &[
    0xCD, 0xDC, 0xD9, 0xE0, 0xE0, 0xDD, 0xD9, 0x00, 0xBE, 0xE3, 0xEB, 0xFF,
];
static SCREDITSTEXT_SHIGEKIMORIMOTO: &[u8] = &[
    0xCD, 0xDC, 0xDD, 0xDB, 0xD9, 0xDF, 0xDD, 0x00, 0xC7, 0xE3, 0xE6, 0xDD, 0xE1, 0xE3, 0xE8, 0xE3,
    0xFF,
];
static SCREDITSTEXT_SHIGERUOHMORI: &[u8] = &[
    0xCD, 0xDC, 0xDD, 0xDB, 0xD9, 0xE6, 0xE9, 0x00, 0xC9, 0xDC, 0xE1, 0xE3, 0xE6, 0xDD, 0xFF,
];
static SCREDITSTEXT_SHUNSUKEKOHORI: &[u8] = &[
    0xCD, 0xDC, 0xE9, 0xE2, 0xE7, 0xE9, 0xDF, 0xD9, 0x00, 0xC5, 0xE3, 0xDC, 0xE3, 0xE6, 0xDD, 0xFF,
];
static SCREDITSTEXT_SHUSAKUEGAMI: &[u8] = &[
    0xCD, 0xDC, 0xE9, 0xE7, 0xD5, 0xDF, 0xE9, 0x00, 0xBF, 0xDB, 0xD5, 0xE1, 0xDD, 0xFF,
];
static SCREDITSTEXT_SOSUKETAMADA: &[u8] = &[
    0xCD, 0xE3, 0xE7, 0xE9, 0xDF, 0xD9, 0x00, 0xCE, 0xD5, 0xE1, 0xD5, 0xD8, 0xD5, 0xFF,
];
static SCREDITSTEXT_SOUICHIYAMAMOTO: &[u8] = &[
    0xCD, 0xE3, 0xE9, 0xDD, 0xD7, 0xDC, 0xDD, 0x00, 0xD3, 0xD5, 0xE1, 0xD5, 0xE1, 0xE3, 0xE8, 0xE3,
    0xFF,
];
static SCREDITSTEXT_SOUNDEFFECTSANDPKMNVOICES: &[u8] = &[
    0xCD, 0xE3, 0xE9, 0xE2, 0xD8, 0x00, 0xBF, 0xDA, 0xDA, 0xD9, 0xD7, 0xE8, 0xE7, 0x00, 0x2D, 0x00,
    0xCA, 0xC9, 0xC5, 0x1B, 0xC7, 0xC9, 0xC8, 0x00, 0xD0, 0xE3, 0xDD, 0xD7, 0xD9, 0xE7, 0xFF,
];
static SCREDITSTEXT_SPECIALTHANKS: &[u8] = &[
    0xCD, 0xE4, 0xD9, 0xD7, 0xDD, 0xD5, 0xE0, 0x00, 0xCE, 0xDC, 0xD5, 0xE2, 0xDF, 0xE7, 0xFF,
];
static SCREDITSTEXT_SUGURUNAKATSUI: &[u8] = &[
    0xCD, 0xE9, 0xDB, 0xE9, 0xE6, 0xE9, 0x00, 0xC8, 0xD5, 0xDF, 0xD5, 0xE8, 0xE7, 0xE9, 0xDD, 0xFF,
];
static SCREDITSTEXT_SUPPORTPROGRAMMERS: &[u8] = &[
    0xCD, 0xE9, 0xE4, 0xE4, 0xE3, 0xE6, 0xE8, 0x00, 0xCA, 0xE6, 0xE3, 0xDB, 0xE6, 0xD5, 0xE1, 0xE1,
    0xD9, 0xE6, 0xE7, 0xFF,
];
static SCREDITSTEXT_TADASHITAKAHASHI: &[u8] = &[
    0xCE, 0xD5, 0xD8, 0xD5, 0xE7, 0xDC, 0xDD, 0x00, 0xCE, 0xD5, 0xDF, 0xD5, 0xDC, 0xD5, 0xE7, 0xDC,
    0xDD, 0xFF,
];
static SCREDITSTEXT_TAKAHIROOHNISHI: &[u8] = &[
    0xCE, 0xD5, 0xDF, 0xD5, 0xDC, 0xDD, 0xE6, 0xE3, 0x00, 0xC9, 0xDC, 0xE2, 0xDD, 0xE7, 0xDC, 0xDD,
    0xFF,
];
static SCREDITSTEXT_TAKANAOKONDO: &[u8] = &[
    0xCE, 0xD5, 0xDF, 0xD5, 0xE2, 0xD5, 0xE3, 0x00, 0xC5, 0xE3, 0xE2, 0xD8, 0xE3, 0xFF,
];
static SCREDITSTEXT_TAKAONAKANO: &[u8] = &[
    0xCE, 0xD5, 0xDF, 0xD5, 0xE3, 0x00, 0xC8, 0xD5, 0xDF, 0xD5, 0xE2, 0xE3, 0xFF,
];
static SCREDITSTEXT_TAKAOUNNO: &[u8] = &[
    0xCE, 0xD5, 0xDF, 0xD5, 0xE3, 0x00, 0xCF, 0xE2, 0xE2, 0xE3, 0xFF,
];
static SCREDITSTEXT_TAKEHIROIZUSHI: &[u8] = &[
    0xCE, 0xD5, 0xDF, 0xD9, 0xDC, 0xDD, 0xE6, 0xE3, 0x00, 0xC3, 0xEE, 0xE9, 0xE7, 0xDC, 0xDD, 0xFF,
];
static SCREDITSTEXT_TAKENORIOHTA: &[u8] = &[
    0xCE, 0xD5, 0xDF, 0xD9, 0xE2, 0xE3, 0xE6, 0xDD, 0x00, 0xC9, 0xDC, 0xE8, 0xD5, 0xFF,
];
static SCREDITSTEXT_TAKESHIKAWACHIMARU: &[u8] = &[
    0xCE, 0xD5, 0xDF, 0xD9, 0xE7, 0xDC, 0xDD, 0x00, 0xC5, 0xD5, 0xEB, 0xD5, 0xD7, 0xDC, 0xDD, 0xE1,
    0xD5, 0xE6, 0xE9, 0xFF,
];
static SCREDITSTEXT_TASKMANAGERS: &[u8] = &[
    0xCE, 0xD5, 0xE7, 0xDF, 0x00, 0xC7, 0xD5, 0xE2, 0xD5, 0xDB, 0xD9, 0xE6, 0xE7, 0xFF,
];
static SCREDITSTEXT_TEIKOSASAKI: &[u8] = &[
    0xCE, 0xD9, 0xDD, 0xDF, 0xE3, 0x00, 0xCD, 0xD5, 0xE7, 0xD5, 0xDF, 0xDD, 0xFF,
];
static SCREDITSTEXT_TERESALILLYGREN: &[u8] = &[
    0xCE, 0xD9, 0xE6, 0xD9, 0xE7, 0xD5, 0x00, 0xC6, 0xDD, 0xE0, 0xE0, 0xED, 0xDB, 0xE6, 0xD9, 0xE2,
    0xFF,
];
static SCREDITSTEXT_TERUKIMURAKAWA: &[u8] = &[
    0xCE, 0xD9, 0xE6, 0xE9, 0xDF, 0xDD, 0x00, 0xC7, 0xE9, 0xE6, 0xD5, 0xDF, 0xD5, 0xEB, 0xD5, 0xFF,
];
static SCREDITSTEXT_TERUYUKISHIMOYAMADA: &[u8] = &[
    0xCE, 0xD9, 0xE6, 0xE9, 0xED, 0xE9, 0xDF, 0xDD, 0x00, 0xCD, 0xDC, 0xDD, 0xE1, 0xE3, 0xED, 0xD5,
    0xE1, 0xD5, 0xD8, 0xD5, 0xFF,
];
static SCREDITSTEXT_TETSUJIOHTA: &[u8] = &[
    0xCE, 0xD9, 0xE8, 0xE7, 0xE9, 0xDE, 0xDD, 0x00, 0xC9, 0xDC, 0xE8, 0xD5, 0xFF,
];
static SCREDITSTEXT_TETSUYAWATANABE: &[u8] = &[
    0xCE, 0xD9, 0xE8, 0xE7, 0xE9, 0xED, 0xD5, 0x00, 0xD1, 0xD5, 0xE8, 0xD5, 0xE2, 0xD5, 0xD6, 0xD9,
    0xFF,
];
static SCREDITSTEXT_TEXTEDITOR: &[u8] = &[
    0xCE, 0xD9, 0xEC, 0xE8, 0x00, 0xBF, 0xD8, 0xDD, 0xE8, 0xE3, 0xE6, 0xFF,
];
static SCREDITSTEXT_THOMASHERTZOG: &[u8] = &[
    0xCE, 0xDC, 0xE3, 0xE1, 0xD5, 0xE7, 0x00, 0xC2, 0xD9, 0xE6, 0xE8, 0xEE, 0xE3, 0xDB, 0xFF,
];
static SCREDITSTEXT_TOMOMICHIOHTA: &[u8] = &[
    0xCE, 0xE3, 0xE1, 0xE3, 0xE1, 0xDD, 0xD7, 0xDC, 0xDD, 0x00, 0xC9, 0xDC, 0xE8, 0xD5, 0xFF,
];
static SCREDITSTEXT_TOMOTAKAKOMURA: &[u8] = &[
    0xCE, 0xE3, 0xE1, 0xE3, 0xE8, 0xD5, 0xDF, 0xD5, 0x00, 0xC5, 0xE3, 0xE1, 0xE9, 0xE6, 0xD5, 0xFF,
];
static SCREDITSTEXT_TOSHINOBUMATSUMIYA: &[u8] = &[
    0xCE, 0xE3, 0xE7, 0xDC, 0xDD, 0xE2, 0xE3, 0xD6, 0xE9, 0x00, 0xC7, 0xD5, 0xE8, 0xE7, 0xE9, 0xE1,
    0xDD, 0xED, 0xD5, 0xFF,
];
static SCREDITSTEXT_TRANSLATOR: &[u8] = &[
    0xCE, 0xE6, 0xD5, 0xE2, 0xE7, 0xE0, 0xD5, 0xE8, 0xE3, 0xE6, 0xFF,
];
static SCREDITSTEXT_TSUNEKAZISHIHARA: &[u8] = &[
    0xCE, 0xE7, 0xE9, 0xE2, 0xD9, 0xDF, 0xD5, 0xEE, 0x00, 0xC3, 0xE7, 0xDC, 0xDD, 0xDC, 0xD5, 0xE6,
    0xD5, 0xFF,
];
static SCREDITSTEXT_WORLDDIRECTOR: &[u8] = &[
    0xD1, 0xE3, 0xE6, 0xE0, 0xD8, 0x00, 0xBE, 0xDD, 0xE6, 0xD9, 0xD7, 0xE8, 0xE3, 0xE6, 0xFF,
];
static SCREDITSTEXT_YOKOWATANABE: &[u8] = &[
    0xD3, 0xE3, 0xDF, 0xE3, 0x00, 0xD1, 0xD5, 0xE8, 0xD5, 0xE2, 0xD5, 0xD6, 0xD9, 0xFF,
];
static SCREDITSTEXT_YOSHINORIMATSUDA: &[u8] = &[
    0xD3, 0xE3, 0xE7, 0xDC, 0xDD, 0xE2, 0xE3, 0xE6, 0xDD, 0x00, 0xC7, 0xD5, 0xE8, 0xE7, 0xE9, 0xD8,
    0xD5, 0xFF,
];
static SCREDITSTEXT_YOSHIOTAJIRI: &[u8] = &[
    0xD3, 0xE3, 0xE7, 0xDC, 0xDD, 0xE3, 0x00, 0xCE, 0xD5, 0xDE, 0xDD, 0xE6, 0xDD, 0xFF,
];
static SCREDITSTEXT_YUICHIROITO: &[u8] = &[
    0xD3, 0xE9, 0xDD, 0xD7, 0xDC, 0xDD, 0xE6, 0xE3, 0x00, 0xC3, 0xE8, 0xE3, 0xFF,
];
static SCREDITSTEXT_YUMIFUNASAKA: &[u8] = &[
    0xD3, 0xE9, 0xE1, 0xDD, 0x00, 0xC0, 0xE9, 0xE2, 0xD5, 0xE7, 0xD5, 0xDF, 0xD5, 0xFF,
];
static SCREDITSTEXT_YURISAKURAI: &[u8] = &[
    0xD3, 0xE9, 0xE6, 0xDD, 0x00, 0xCD, 0xD5, 0xDF, 0xE9, 0xE6, 0xD5, 0xDD, 0xFF,
];

pub const PAGE_COUNT: usize = 57;
pub const ENTRIES_PER_PAGE: usize = 5;
pub static PAGES: [[Line; ENTRIES_PER_PAGE]; PAGE_COUNT] = [
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_PKMNEMERALDVERSION,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_CREDITS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_DIRECTOR,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SHIGEKIMORIMOTO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_ARTDIRECTOR,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KENSUGIMORI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_WORLDDIRECTOR,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_JUNICHIMASUDA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_LEADPROGRAMMER,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HISASHISOGABE,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_LEADGRAPHICARTIST,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_MOTOFUMIFUJIWARA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_PROGRAMMERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HISASHISOGABE,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TOMOMICHIOHTA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_NOZOMUSAITO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_PROGRAMMERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_AKITOMORI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HIROYUKINAKAMURA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_MASAOTAYA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_PROGRAMMERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SATOSHINOHARA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_MIYUKIIWASAWA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_YOSHINORIMATSUDA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KEITAKAGAYA,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_PROGRAMMERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TETSUYAWATANABE,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SOSUKETAMADA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TAKENORIOHTA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_GRAPHICDESIGNERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_MOTOFUMIFUJIWARA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SATOSHIOHTA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_GRAPHICDESIGNERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KENKICHITOYAMA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_ASUKAIWASHITA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TAKAOUNNO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_GRAPHICDESIGNERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KENSUGIMORI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HIRONOBUYOSHIDA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_AIMITOMITA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KANAKOEO,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_MUSICCOMPOSITION,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_GOICHINOSE,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_JUNICHIMASUDA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_MORIKAZUAOKI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HITOMISATO,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_SOUNDEFFECTSANDPKMNVOICES,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_GOICHINOSE,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_MORIKAZUAOKI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_GAMEDESIGNERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SHIGEKIMORIMOTO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TERUYUKISHIMOYAMADA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TAKESHIKAWACHIMARU,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_AKIHITOTOMISAWA,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_GAMEDESIGNERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SUGURUNAKATSUI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TETSUJIOHTA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HITOMISATO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KENJIMATSUSHIMA,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_GAMEDESIGNERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_JUNICHIMASUDA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KOJINISHINO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SHIGERUOHMORI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TADASHITAKAHASHI,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_SCENARIOPLOT,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_AKIHITOTOMISAWA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_JUNICHIMASUDA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KOJINISHINO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_SCENARIO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_AKIHITOTOMISAWA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HITOMISATO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TOSHINOBUMATSUMIYA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_SCRIPTDESIGNERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TOMOMICHIOHTA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SATOSHINOHARA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_MAPDESIGNERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SUGURUNAKATSUI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TERUYUKISHIMOYAMADA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SHIGERUOHMORI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TETSUJIOHTA,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_BATTLEFRONTIERDATA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TETSUJIOHTA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_PARAMETRICDESIGNERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TERUYUKISHIMOYAMADA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SHIGEKIMORIMOTO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TETSUJIOHTA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KOJINISHINO,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_POKEDEXTEXT,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KENJIMATSUSHIMA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_ENVANDTOOLPGRMS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HISASHISOGABE,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SOSUKETAMADA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HIROYUKINAKAMURA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_AKITOMORI,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_PKMNDESIGNERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KENSUGIMORI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_MOTOFUMIFUJIWARA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SHIGEKIMORIMOTO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_PKMNDESIGNERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HIRONOBUYOSHIDA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SATOSHIOHTA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_ASUKAIWASHITA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_PKMNDESIGNERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TAKAOUNNO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KANAKOEO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_AIMITOMITA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_PKMNDESIGNERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_ATSUKONISHIDA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_MUNEOSAITO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_RENAYOSHIKAWA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_JUNOKUTANI,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_SUPPORTPROGRAMMERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SATOSHIMITSUHARA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_DAISUKEHOSHINO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_NCLPRODUCTTESTING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_NCLSUPERMARIOCLUB,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_PACKAGEANDMANUAL,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KENSUGIMORI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_SPECIALTHANKS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KENJITOMINAGA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HIROKIENOMOTO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_SPECIALTHANKS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KAZUYASUYAMA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KENJIROITO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_MICHIKOTAKIZAWA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_MAKIKOTAKADA,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_SPECIALTHANKS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_MIKIKOOHHASHI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TAKANAOKONDO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_RUIKAWAGUCHI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_SPECIALTHANKS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TAKAHIROOHNISHI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_NORIHIDEOKAMURA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SHUNSUKEKOHORI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_INFOSUPERVISORS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KAZUYUKITERADA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_YURISAKURAI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_YUMIFUNASAKA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_NAOKOYANASE,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_ARTWORK,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SACHIKONAKAMICHI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_FUJIKONOMURA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_ARTWORK,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HIDEYUKINAKAJIMA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HIDENORISAEKI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_ARTWORK,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_YOKOWATANABE,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SAKAEKIMURA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_CHIAKISHINKAI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_COORDINATORS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KAZUKIYOSHIHARA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_AKIRAKINASHI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_RETSUJINOMOTO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_ENGLISHVERSION,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HIRONAKAMURA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SETHMCMAHILL,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_TRANSLATOR,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_NOBOGASAWARA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_TEXTEDITOR,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TERESALILLYGREN,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_NCLCOORDINATOR,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KIMIKONAKAMICHI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_PROGRAMMERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TERUKIMURAKAWA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SOUICHIYAMAMOTO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_YUICHIROITO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_AKIRAKINASHI,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_GRAPHICDESIGNER,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_AKIRAKINASHI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_ENVANDTOOLPGRMS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TERUKIMURAKAWA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SOUICHIYAMAMOTO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_KIMIKONAKAMICHI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_NOAPRODUCTTESTING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_THOMASHERTZOG,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_ERIKJOHNSON,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_MIKAKUROSAWA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_BRAILLECODECHECK,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_NATIONALFEDERATIONBLIND,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_PATRICIAAMAURER,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_JAPANBRAILLELIBRARY,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EUROPEANBLINDUNION,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_BRAILLECODECHECK,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_AUSTRALIANBRAILLEAUTHORITY,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_ROYALNEWZEALANDFEDERATIONBLIND,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_SPECIALTHANKS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HIROYUKIUESUGI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_MOTOYASUTOJIMA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_NICOLAPRATTBARLOW,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SHELLIEDOW,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_TASKMANAGERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_AZUSATAJIMA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SHUSAKUEGAMI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: true,
            text: SCREDITSTEXT_PRODUCERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HIROYUKIJINNAI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HITOSHIYAMAGAMI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_GAKUJINOMOTO,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_HIROAKITSURU,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_EXECUTIVEDIRECTOR,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SATOSHITAJIRI,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_EXECPRODUCERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_SATORUIWATA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
    [
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: true,
            text: SCREDITSTEXT_EXECPRODUCERS,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_TSUNEKAZISHIHARA,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
        Line {
            title: false,
            text: SCREDITSTEXT_EMPTYSTRING,
        },
    ],
];

/// First line of page 0 (`sCreditsText_PkmnEmeraldVersion`).
pub static FIRST_PAGE_LINE: &[u8] = SCREDITSTEXT_PKMNEMERALDVERSION;
