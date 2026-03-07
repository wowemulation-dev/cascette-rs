//! Privacy-friendly geolocation via OS timezone.
//!
//! Derives ISO 3166-1 country codes and Blizzard CDN region from the system's
//! IANA timezone setting. This avoids network-based GeoIP lookups while still
//! providing sensible defaults for `.build.info` `acct-` and `geoip-` tags.
//!
//! The mapping uses the IANA zone.tab database (timezone -> alpha-2 country)
//! combined with the `isocountry` crate (alpha-2 -> alpha-3 conversion).

use isocountry::CountryCode;
use tracing::debug;

/// Geolocation result from OS timezone detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoDefaults {
    /// ISO 3166-1 alpha-2 country code (e.g., "BG", "DE").
    pub alpha2: String,
    /// ISO 3166-1 alpha-3 country code (e.g., "BGR", "DEU").
    pub alpha3: String,
    /// Blizzard CDN region code: US, EU, KR, TW, CN, or SG.
    pub blizzard_region: String,
}

/// Detect country and Blizzard region from the OS timezone.
///
/// Reads the system timezone via `iana-time-zone`, maps it to a country code
/// using the IANA zone.tab data, then derives the alpha-3 code and Blizzard
/// CDN region.
///
/// Returns `None` if:
/// - The timezone cannot be read (unusual OS configuration)
/// - The timezone is not in the zone.tab mapping (custom/synthetic timezone)
/// - The alpha-2 code has no alpha-3 counterpart (should not happen for valid codes)
pub fn detect_geo_defaults() -> Option<GeoDefaults> {
    let tz = iana_time_zone::get_timezone().ok()?;
    detect_geo_defaults_from_timezone(&tz)
}

/// Derive geolocation from a specific IANA timezone string.
///
/// Exposed for testing — production code should use [`detect_geo_defaults`].
pub fn detect_geo_defaults_from_timezone(timezone: &str) -> Option<GeoDefaults> {
    let alpha2 = timezone_to_alpha2(timezone)?;
    let country = CountryCode::for_alpha2(alpha2).ok()?;
    let alpha3 = country.alpha3();
    let region = country_to_blizzard_region(alpha2);

    debug!(
        timezone,
        alpha2, alpha3, region, "geolocation defaults from timezone"
    );

    Some(GeoDefaults {
        alpha2: alpha2.to_string(),
        alpha3: alpha3.to_string(),
        blizzard_region: region.to_string(),
    })
}

/// Map an ISO 3166-1 alpha-2 country code to a Blizzard CDN region.
///
/// Returns one of: `"US"`, `"EU"`, `"KR"`, `"TW"`, `"CN"`, `"SG"`.
/// Unknown codes default to `"US"` (matching Blizzard Agent behavior).
pub fn country_to_blizzard_region(alpha2: &str) -> &'static str {
    match alpha2 {
        // Dedicated regions
        "KR" => "KR",
        "TW" => "TW",
        "CN" => "CN",

        // SG region: Southeast Asia, East Asia (except KR/TW/CN), Oceania
        "AU" | "BN" | "FJ" | "GU" | "HK" | "ID" | "JP" | "KH" | "LA" | "MM" | "MN" | "MO"
        | "MY" | "NC" | "NZ" | "PG" | "PH" | "SG" | "TH" | "VN" | "WS" | "TO" | "TV"
        | "VU" | "SB" | "MH" | "FM" | "PW" | "KI" | "NR" | "TL" | "BT" | "NP" | "BD"
        | "LK" | "MV" => "SG",

        // EU region: Europe, Africa, Middle East, Central Asia, South Asia (India/Pakistan)
        "AD" | "AE" | "AF" | "AL" | "AM" | "AO" | "AT" | "AZ" | "BA" | "BE" | "BF" | "BG"
        | "BH" | "BI" | "BJ" | "BW" | "BY" | "CD" | "CF" | "CG" | "CH" | "CI" | "CM"
        | "CV" | "CY" | "CZ" | "DE" | "DJ" | "DK" | "DZ" | "EE" | "EG" | "EH" | "ER"
        | "ES" | "ET" | "FI" | "FO" | "FR" | "GA" | "GB" | "GE" | "GH" | "GI" | "GL"
        | "GM" | "GN" | "GQ" | "GR" | "GW" | "HR" | "HU" | "IE" | "IL" | "IN" | "IQ"
        | "IR" | "IS" | "IT" | "JO" | "KE" | "KG" | "KZ" | "LB" | "LI" | "LR" | "LS"
        | "LT" | "LU" | "LV" | "LY" | "MA" | "MC" | "MD" | "ME" | "MG" | "MK" | "ML"
        | "MR" | "MT" | "MU" | "MW" | "MZ" | "NA" | "NE" | "NG" | "NL" | "NO" | "OM"
        | "PK" | "PL" | "PS" | "PT" | "QA" | "RE" | "RO" | "RS" | "RU" | "RW" | "SA"
        | "SC" | "SD" | "SE" | "SI" | "SK" | "SL" | "SM" | "SN" | "SO" | "SS" | "ST"
        | "SY" | "SZ" | "TD" | "TG" | "TJ" | "TM" | "TN" | "TR" | "TZ" | "UA" | "UG"
        | "UZ" | "VA" | "XK" | "YE" | "ZA" | "ZM" | "ZW" => "EU",

        // US region: Americas (default for anything else)
        _ => "US",
    }
}

/// Map an IANA timezone name to its ISO 3166-1 alpha-2 country code.
///
/// Data sourced from the IANA Time Zone Database zone.tab / zone1970.tab.
/// Each canonical timezone maps to exactly one country.
///
/// Returns `None` for synthetic timezones (UTC, GMT, Etc/*) and unknown names.
#[allow(clippy::too_many_lines)]
fn timezone_to_alpha2(timezone: &str) -> Option<&'static str> {
    // Grouped by country code to satisfy clippy::match_same_arms.
    // Countries with multiple timezones have all entries merged into one arm.
    Some(match timezone {
        // AD - Andorra
        "Europe/Andorra" => "AD",
        // AE - United Arab Emirates
        "Asia/Dubai" => "AE",
        // AF - Afghanistan
        "Asia/Kabul" => "AF",
        // AG - Antigua and Barbuda
        "America/Antigua" => "AG",
        // AI - Anguilla
        "America/Anguilla" => "AI",
        // AL - Albania
        "Europe/Tirane" => "AL",
        // AM - Armenia
        "Asia/Yerevan" => "AM",
        // AO - Angola
        "Africa/Luanda" => "AO",
        // AQ - Antarctica (unaffiliated stations)
        "Antarctica/DumontDUrville" | "Antarctica/Palmer" | "Antarctica/Rothera"
        | "Antarctica/Syowa" | "Antarctica/Troll" | "Antarctica/Vostok" => "AQ",
        // AR - Argentina
        "America/Argentina/Buenos_Aires" | "America/Buenos_Aires"
        | "America/Argentina/Catamarca" | "America/Catamarca"
        | "America/Argentina/ComodRivadavia"
        | "America/Argentina/Cordoba" | "America/Cordoba" | "America/Rosario"
        | "America/Argentina/Jujuy" | "America/Jujuy"
        | "America/Argentina/La_Rioja"
        | "America/Argentina/Mendoza" | "America/Mendoza"
        | "America/Argentina/Rio_Gallegos" | "America/Argentina/Salta"
        | "America/Argentina/San_Juan" | "America/Argentina/San_Luis"
        | "America/Argentina/Tucuman" | "America/Argentina/Ushuaia" => "AR",
        // AS - American Samoa
        "Pacific/Pago_Pago" | "Pacific/Samoa" => "AS",
        // AT - Austria
        "Europe/Vienna" => "AT",
        // AU - Australia
        "Antarctica/Casey" | "Antarctica/Davis" | "Antarctica/Macquarie"
        | "Antarctica/Mawson"
        | "Australia/ACT" | "Australia/Canberra" | "Australia/NSW" | "Australia/Sydney"
        | "Australia/Adelaide" | "Australia/South"
        | "Australia/Brisbane" | "Australia/Queensland"
        | "Australia/Broken_Hill" | "Australia/Yancowinna"
        | "Australia/Currie"
        | "Australia/Darwin" | "Australia/North"
        | "Australia/Eucla"
        | "Australia/Hobart" | "Australia/Tasmania"
        | "Australia/LHI" | "Australia/Lord_Howe"
        | "Australia/Lindeman"
        | "Australia/Melbourne" | "Australia/Victoria"
        | "Australia/Perth" | "Australia/West" => "AU",
        // AW - Aruba
        "America/Aruba" => "AW",
        // AX - Aland Islands
        "Europe/Mariehamn" => "AX",
        // AZ - Azerbaijan
        "Asia/Baku" => "AZ",
        // BA - Bosnia and Herzegovina
        "Europe/Sarajevo" => "BA",
        // BB - Barbados
        "America/Barbados" => "BB",
        // BD - Bangladesh
        "Asia/Dacca" | "Asia/Dhaka" => "BD",
        // BE - Belgium
        "Europe/Brussels" => "BE",
        // BF - Burkina Faso
        "Africa/Ouagadougou" => "BF",
        // BG - Bulgaria
        "Europe/Sofia" => "BG",
        // BH - Bahrain
        "Asia/Bahrain" => "BH",
        // BI - Burundi
        "Africa/Bujumbura" => "BI",
        // BJ - Benin
        "Africa/Porto-Novo" => "BJ",
        // BL - Saint Barthelemy
        "America/St_Barthelemy" => "BL",
        // BM - Bermuda
        "Atlantic/Bermuda" => "BM",
        // BN - Brunei
        "Asia/Brunei" => "BN",
        // BO - Bolivia
        "America/La_Paz" => "BO",
        // BQ - Bonaire
        "America/Kralendijk" => "BQ",
        // BR - Brazil
        "America/Araguaina" | "America/Bahia" | "America/Belem" | "America/Boa_Vista"
        | "America/Campo_Grande" | "America/Cuiaba" | "America/Eirunepe"
        | "America/Fortaleza" | "America/Maceio" | "America/Manaus" | "America/Noronha"
        | "America/Porto_Velho" | "America/Recife"
        | "America/Rio_Branco" | "America/Porto_Acre"
        | "America/Santarem" | "America/Sao_Paulo" => "BR",
        // BS - Bahamas
        "America/Nassau" => "BS",
        // BT - Bhutan
        "Asia/Thimbu" | "Asia/Thimphu" => "BT",
        // BW - Botswana
        "Africa/Gaborone" => "BW",
        // BY - Belarus
        "Europe/Minsk" => "BY",
        // BZ - Belize
        "America/Belize" => "BZ",
        // CA - Canada
        "America/Atikokan" | "America/Coral_Harbour" | "America/Blanc-Sablon"
        | "America/Cambridge_Bay" | "America/Creston"
        | "America/Dawson" | "America/Dawson_Creek"
        | "America/Edmonton" | "America/Fort_Nelson"
        | "America/Glace_Bay" | "America/Goose_Bay" | "America/Halifax"
        | "America/Inuvik" | "America/Iqaluit"
        | "America/Moncton" | "America/Montreal" | "America/Toronto"
        | "America/Nipigon" | "America/Pangnirtung"
        | "America/Rainy_River" | "America/Rankin_Inlet"
        | "America/Regina" | "America/Resolute"
        | "America/St_Johns" | "America/Swift_Current"
        | "America/Thunder_Bay" | "America/Vancouver"
        | "America/Whitehorse" | "America/Winnipeg" | "America/Yellowknife" => "CA",
        // CC - Cocos Islands
        "Indian/Cocos" => "CC",
        // CD - DR Congo
        "Africa/Kinshasa" | "Africa/Lubumbashi" => "CD",
        // CF - Central African Republic
        "Africa/Bangui" => "CF",
        // CG - Republic of the Congo
        "Africa/Brazzaville" => "CG",
        // CH - Switzerland
        "Europe/Zurich" => "CH",
        // CI - Cote d'Ivoire
        "Africa/Abidjan" => "CI",
        // CK - Cook Islands
        "Pacific/Rarotonga" => "CK",
        // CL - Chile
        "America/Punta_Arenas" | "America/Santiago" | "Pacific/Easter" => "CL",
        // CM - Cameroon
        "Africa/Douala" => "CM",
        // CN - China
        "Asia/Chongqing" | "Asia/Chungking" | "Asia/Harbin"
        | "Asia/Kashgar" | "Asia/Urumqi" | "Asia/Shanghai" => "CN",
        // CO - Colombia
        "America/Bogota" => "CO",
        // CR - Costa Rica
        "America/Costa_Rica" => "CR",
        // CU - Cuba
        "America/Havana" => "CU",
        // CV - Cape Verde
        "Atlantic/Cape_Verde" => "CV",
        // CW - Curacao
        "America/Curacao" => "CW",
        // CX - Christmas Island
        "Indian/Christmas" => "CX",
        // CY - Cyprus
        "Asia/Famagusta" | "Asia/Nicosia" | "Europe/Nicosia" => "CY",
        // CZ - Czech Republic
        "Europe/Prague" => "CZ",
        // DE - Germany
        "Europe/Berlin" | "Europe/Busingen" => "DE",
        // DJ - Djibouti
        "Africa/Djibouti" => "DJ",
        // DK - Denmark
        "Europe/Copenhagen" => "DK",
        // DM - Dominica
        "America/Dominica" => "DM",
        // DO - Dominican Republic
        "America/Santo_Domingo" => "DO",
        // DZ - Algeria
        "Africa/Algiers" => "DZ",
        // EC - Ecuador
        "America/Guayaquil" | "Pacific/Galapagos" => "EC",
        // EE - Estonia
        "Europe/Tallinn" => "EE",
        // EG - Egypt
        "Africa/Cairo" => "EG",
        // EH - Western Sahara
        "Africa/El_Aaiun" => "EH",
        // ER - Eritrea
        "Africa/Asmara" | "Africa/Asmera" => "ER",
        // ES - Spain
        "Africa/Ceuta" | "Atlantic/Canary" | "Europe/Madrid" => "ES",
        // ET - Ethiopia
        "Africa/Addis_Ababa" => "ET",
        // FI - Finland
        "Europe/Helsinki" => "FI",
        // FJ - Fiji
        "Pacific/Fiji" => "FJ",
        // FK - Falkland Islands
        "Atlantic/Stanley" => "FK",
        // FM - Micronesia
        "Pacific/Chuuk" | "Pacific/Truk" | "Pacific/Yap"
        | "Pacific/Kosrae" | "Pacific/Pohnpei" | "Pacific/Ponape" => "FM",
        // FO - Faroe Islands
        "Atlantic/Faroe" | "Atlantic/Faeroe" => "FO",
        // FR - France
        "Europe/Paris" => "FR",
        // GA - Gabon
        "Africa/Libreville" => "GA",
        // GB - United Kingdom
        "Europe/Belfast" | "Europe/London" => "GB",
        // GD - Grenada
        "America/Grenada" => "GD",
        // GE - Georgia
        "Asia/Tbilisi" => "GE",
        // GF - French Guiana
        "America/Cayenne" => "GF",
        // GG - Guernsey
        "Europe/Guernsey" => "GG",
        // GH - Ghana
        "Africa/Accra" => "GH",
        // GI - Gibraltar
        "Europe/Gibraltar" => "GI",
        // GL - Greenland
        "America/Danmarkshavn" | "America/Godthab" | "America/Nuuk"
        | "America/Scoresbysund" | "America/Ittoqqortoormiit" | "America/Thule" => "GL",
        // GM - Gambia
        "Africa/Banjul" => "GM",
        // GN - Guinea
        "Africa/Conakry" => "GN",
        // GP - Guadeloupe
        "America/Guadeloupe" => "GP",
        // GQ - Equatorial Guinea
        "Africa/Malabo" => "GQ",
        // GR - Greece
        "Europe/Athens" => "GR",
        // GS - South Georgia
        "Atlantic/South_Georgia" => "GS",
        // GT - Guatemala
        "America/Guatemala" => "GT",
        // GU - Guam
        "Pacific/Guam" => "GU",
        // GW - Guinea-Bissau
        "Africa/Bissau" => "GW",
        // GY - Guyana
        "America/Guyana" => "GY",
        // HK - Hong Kong
        "Asia/Hong_Kong" => "HK",
        // HN - Honduras
        "America/Tegucigalpa" => "HN",
        // HR - Croatia
        "Europe/Zagreb" => "HR",
        // HT - Haiti
        "America/Port-au-Prince" => "HT",
        // HU - Hungary
        "Europe/Budapest" => "HU",
        // ID - Indonesia
        "Asia/Jakarta" | "Asia/Jayapura" | "Asia/Makassar" | "Asia/Ujung_Pandang"
        | "Asia/Pontianak" => "ID",
        // IE - Ireland
        "Europe/Dublin" => "IE",
        // IL - Israel
        "Asia/Jerusalem" | "Asia/Tel_Aviv" => "IL",
        // IM - Isle of Man
        "Europe/Isle_of_Man" => "IM",
        // IN - India
        "Asia/Calcutta" | "Asia/Kolkata" => "IN",
        // IO - British Indian Ocean Territory
        "Indian/Chagos" => "IO",
        // IQ - Iraq
        "Asia/Baghdad" => "IQ",
        // IR - Iran
        "Asia/Tehran" => "IR",
        // IS - Iceland
        "Atlantic/Reykjavik" => "IS",
        // IT - Italy
        "Europe/Rome" => "IT",
        // JE - Jersey
        "Europe/Jersey" => "JE",
        // JM - Jamaica
        "America/Jamaica" => "JM",
        // JO - Jordan
        "Asia/Amman" => "JO",
        // JP - Japan
        "Asia/Tokyo" => "JP",
        // KE - Kenya
        "Africa/Nairobi" => "KE",
        // KG - Kyrgyzstan
        "Asia/Bishkek" => "KG",
        // KH - Cambodia
        "Asia/Phnom_Penh" => "KH",
        // KI - Kiribati
        "Pacific/Enderbury" | "Pacific/Kanton" | "Pacific/Kiritimati"
        | "Pacific/Tarawa" => "KI",
        // KM - Comoros
        "Indian/Comoro" => "KM",
        // KN - Saint Kitts and Nevis
        "America/St_Kitts" => "KN",
        // KP - North Korea
        "Asia/Pyongyang" => "KP",
        // KR - South Korea
        "Asia/Seoul" => "KR",
        // KW - Kuwait
        "Asia/Kuwait" => "KW",
        // KY - Cayman Islands
        "America/Cayman" => "KY",
        // KZ - Kazakhstan
        "Asia/Almaty" | "Asia/Aqtau" | "Asia/Aqtobe" | "Asia/Atyrau"
        | "Asia/Oral" | "Asia/Qostanay" | "Asia/Qyzylorda" => "KZ",
        // LA - Laos
        "Asia/Vientiane" => "LA",
        // LB - Lebanon
        "Asia/Beirut" => "LB",
        // LC - Saint Lucia
        "America/St_Lucia" => "LC",
        // LI - Liechtenstein
        "Europe/Vaduz" => "LI",
        // LK - Sri Lanka
        "Asia/Colombo" => "LK",
        // LR - Liberia
        "Africa/Monrovia" => "LR",
        // LS - Lesotho
        "Africa/Maseru" => "LS",
        // LT - Lithuania
        "Europe/Vilnius" => "LT",
        // LU - Luxembourg
        "Europe/Luxembourg" => "LU",
        // LV - Latvia
        "Europe/Riga" => "LV",
        // LY - Libya
        "Africa/Tripoli" => "LY",
        // MA - Morocco
        "Africa/Casablanca" => "MA",
        // MC - Monaco
        "Europe/Monaco" => "MC",
        // MD - Moldova
        "Europe/Chisinau" | "Europe/Tiraspol" => "MD",
        // ME - Montenegro
        "Europe/Podgorica" => "ME",
        // MF - Saint Martin
        "America/Marigot" => "MF",
        // MG - Madagascar
        "Indian/Antananarivo" => "MG",
        // MH - Marshall Islands
        "Pacific/Kwajalein" | "Pacific/Majuro" => "MH",
        // MK - North Macedonia
        "Europe/Skopje" => "MK",
        // ML - Mali
        "Africa/Bamako" | "Africa/Timbuktu" => "ML",
        // MM - Myanmar
        "Asia/Rangoon" | "Asia/Yangon" => "MM",
        // MN - Mongolia
        "Asia/Choibalsan" | "Asia/Hovd" | "Asia/Ulaanbaatar" | "Asia/Ulan_Bator" => "MN",
        // MO - Macao
        "Asia/Macao" | "Asia/Macau" => "MO",
        // MP - Northern Mariana Islands
        "Pacific/Saipan" => "MP",
        // MQ - Martinique
        "America/Martinique" => "MQ",
        // MR - Mauritania
        "Africa/Nouakchott" => "MR",
        // MS - Montserrat
        "America/Montserrat" => "MS",
        // MT - Malta
        "Europe/Malta" => "MT",
        // MU - Mauritius
        "Indian/Mauritius" => "MU",
        // MV - Maldives
        "Indian/Maldives" => "MV",
        // MW - Malawi
        "Africa/Blantyre" => "MW",
        // MX - Mexico
        "America/Bahia_Banderas" | "America/Cancun" | "America/Chihuahua"
        | "America/Ciudad_Juarez" | "America/Ensenada" | "America/Tijuana"
        | "America/Hermosillo" | "America/Matamoros" | "America/Mazatlan"
        | "America/Merida" | "America/Mexico_City" | "America/Monterrey"
        | "America/Ojinaga" => "MX",
        // MY - Malaysia
        "Asia/Kuala_Lumpur" | "Asia/Kuching" => "MY",
        // MZ - Mozambique
        "Africa/Maputo" => "MZ",
        // NA - Namibia
        "Africa/Windhoek" => "NA",
        // NC - New Caledonia
        "Pacific/Noumea" => "NC",
        // NE - Niger
        "Africa/Niamey" => "NE",
        // NF - Norfolk Island
        "Pacific/Norfolk" => "NF",
        // NG - Nigeria
        "Africa/Lagos" => "NG",
        // NI - Nicaragua
        "America/Managua" => "NI",
        // NL - Netherlands
        "Europe/Amsterdam" => "NL",
        // NO - Norway
        "Arctic/Longyearbyen" | "Atlantic/Jan_Mayen" | "Europe/Oslo" => "NO",
        // NP - Nepal
        "Asia/Kathmandu" | "Asia/Katmandu" => "NP",
        // NR - Nauru
        "Pacific/Nauru" => "NR",
        // NU - Niue
        "Pacific/Niue" => "NU",
        // NZ - New Zealand
        "Antarctica/McMurdo" | "Antarctica/South_Pole"
        | "Pacific/Auckland" | "Pacific/Chatham" => "NZ",
        // OM - Oman
        "Asia/Muscat" => "OM",
        // PA - Panama
        "America/Panama" => "PA",
        // PE - Peru
        "America/Lima" => "PE",
        // PF - French Polynesia
        "Pacific/Gambier" | "Pacific/Marquesas" | "Pacific/Tahiti" => "PF",
        // PG - Papua New Guinea
        "Pacific/Bougainville" | "Pacific/Port_Moresby" => "PG",
        // PH - Philippines
        "Asia/Manila" => "PH",
        // PK - Pakistan
        "Asia/Karachi" => "PK",
        // PL - Poland
        "Europe/Warsaw" => "PL",
        // PM - Saint Pierre and Miquelon
        "America/Miquelon" => "PM",
        // PN - Pitcairn Islands
        "Pacific/Pitcairn" => "PN",
        // PR - Puerto Rico
        "America/Puerto_Rico" => "PR",
        // PS - Palestine
        "Asia/Gaza" | "Asia/Hebron" => "PS",
        // PT - Portugal
        "Atlantic/Azores" | "Atlantic/Madeira" | "Europe/Lisbon" => "PT",
        // PW - Palau
        "Pacific/Palau" => "PW",
        // PY - Paraguay
        "America/Asuncion" => "PY",
        // QA - Qatar
        "Asia/Qatar" => "QA",
        // RE - Reunion
        "Indian/Reunion" => "RE",
        // RO - Romania
        "Europe/Bucharest" => "RO",
        // RS - Serbia
        "Europe/Belgrade" => "RS",
        // RU - Russia
        "Asia/Anadyr" | "Asia/Barnaul" | "Asia/Chita" | "Asia/Irkutsk"
        | "Asia/Kamchatka" | "Asia/Khandyga" | "Asia/Krasnoyarsk"
        | "Asia/Magadan" | "Asia/Novokuznetsk" | "Asia/Novosibirsk"
        | "Asia/Omsk" | "Asia/Sakhalin" | "Asia/Srednekolymsk"
        | "Asia/Tomsk" | "Asia/Ust-Nera" | "Asia/Vladivostok"
        | "Asia/Yakutsk" | "Asia/Yekaterinburg"
        | "Europe/Astrakhan" | "Europe/Kaliningrad" | "Europe/Kirov"
        | "Europe/Moscow" | "Europe/Samara" | "Europe/Saratov"
        | "Europe/Ulyanovsk" | "Europe/Volgograd" => "RU",
        // RW - Rwanda
        "Africa/Kigali" => "RW",
        // SA - Saudi Arabia
        "Asia/Riyadh" => "SA",
        // SB - Solomon Islands
        "Pacific/Guadalcanal" => "SB",
        // SC - Seychelles
        "Indian/Mahe" => "SC",
        // SD - Sudan
        "Africa/Khartoum" => "SD",
        // SE - Sweden
        "Europe/Stockholm" => "SE",
        // SG - Singapore
        "Asia/Singapore" => "SG",
        // SH - Saint Helena
        "Atlantic/St_Helena" => "SH",
        // SI - Slovenia
        "Europe/Ljubljana" => "SI",
        // SK - Slovakia
        "Europe/Bratislava" => "SK",
        // SL - Sierra Leone
        "Africa/Freetown" => "SL",
        // SM - San Marino
        "Europe/San_Marino" => "SM",
        // SN - Senegal
        "Africa/Dakar" => "SN",
        // SO - Somalia
        "Africa/Mogadishu" => "SO",
        // SR - Suriname
        "America/Paramaribo" => "SR",
        // SS - South Sudan
        "Africa/Juba" => "SS",
        // ST - Sao Tome and Principe
        "Africa/Sao_Tome" => "ST",
        // SV - El Salvador
        "America/El_Salvador" => "SV",
        // SX - Sint Maarten
        "America/Lower_Princes" => "SX",
        // SY - Syria
        "Asia/Damascus" => "SY",
        // SZ - Eswatini
        "Africa/Mbabane" => "SZ",
        // TC - Turks and Caicos
        "America/Grand_Turk" => "TC",
        // TD - Chad
        "Africa/Ndjamena" => "TD",
        // TF - French Southern Territories
        "Indian/Kerguelen" => "TF",
        // TG - Togo
        "Africa/Lome" => "TG",
        // TH - Thailand
        "Asia/Bangkok" => "TH",
        // TJ - Tajikistan
        "Asia/Dushanbe" => "TJ",
        // TK - Tokelau
        "Pacific/Fakaofo" => "TK",
        // TL - Timor-Leste
        "Asia/Dili" => "TL",
        // TM - Turkmenistan
        "Asia/Ashgabat" | "Asia/Ashkhabad" => "TM",
        // TN - Tunisia
        "Africa/Tunis" => "TN",
        // TO - Tonga
        "Pacific/Tongatapu" => "TO",
        // TR - Turkey
        "Asia/Istanbul" | "Europe/Istanbul" => "TR",
        // TT - Trinidad and Tobago
        "America/Port_of_Spain" => "TT",
        // TV - Tuvalu
        "Pacific/Funafuti" => "TV",
        // TW - Taiwan
        "Asia/Taipei" => "TW",
        // TZ - Tanzania
        "Africa/Dar_es_Salaam" => "TZ",
        // UA - Ukraine
        "Europe/Kiev" | "Europe/Kyiv" | "Europe/Uzhgorod" | "Europe/Zaporozhye"
        | "Europe/Simferopol" => "UA",
        // UG - Uganda
        "Africa/Kampala" => "UG",
        // US - United States
        "America/Adak" | "America/Anchorage" | "America/Boise" | "America/Chicago"
        | "America/Denver" | "America/Detroit"
        | "America/Fort_Wayne" | "America/Indiana/Indianapolis" | "America/Indianapolis"
        | "America/Indiana/Knox" | "America/Knox_IN"
        | "America/Indiana/Marengo" | "America/Indiana/Petersburg"
        | "America/Indiana/Tell_City" | "America/Indiana/Vevay"
        | "America/Indiana/Vincennes" | "America/Indiana/Winamac"
        | "America/Juneau"
        | "America/Kentucky/Louisville" | "America/Louisville"
        | "America/Kentucky/Monticello"
        | "America/Los_Angeles" | "America/Menominee" | "America/Metlakatla"
        | "America/New_York" | "America/Nome"
        | "America/North_Dakota/Beulah" | "America/North_Dakota/Center"
        | "America/North_Dakota/New_Salem"
        | "America/Phoenix" | "America/Shiprock" | "America/Sitka"
        | "America/Yakutat"
        | "Pacific/Honolulu" | "Pacific/Johnston" | "Pacific/Midway"
        | "Pacific/Wake" => "US",
        // UY - Uruguay
        "America/Montevideo" => "UY",
        // UZ - Uzbekistan
        "Asia/Samarkand" | "Asia/Tashkent" => "UZ",
        // VA - Vatican City
        "Europe/Vatican" => "VA",
        // VC - Saint Vincent and the Grenadines
        "America/St_Vincent" => "VC",
        // VE - Venezuela
        "America/Caracas" => "VE",
        // VG - British Virgin Islands
        "America/Tortola" => "VG",
        // VI - US Virgin Islands
        "America/St_Thomas" | "America/Virgin" => "VI",
        // VN - Vietnam
        "Asia/Ho_Chi_Minh" | "Asia/Saigon" => "VN",
        // VU - Vanuatu
        "Pacific/Efate" => "VU",
        // WF - Wallis and Futuna
        "Pacific/Wallis" => "WF",
        // WS - Samoa
        "Pacific/Apia" => "WS",
        // YE - Yemen
        "Asia/Aden" => "YE",
        // YT - Mayotte
        "Indian/Mayotte" => "YT",
        // ZA - South Africa
        "Africa/Johannesburg" => "ZA",
        // ZM - Zambia
        "Africa/Lusaka" => "ZM",
        // ZW - Zimbabwe
        "Africa/Harare" => "ZW",

        // Unrecognized or synthetic timezones (UTC, GMT, Etc/*)
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- timezone_to_alpha2 ---

    #[test]
    fn timezone_sofia_is_bulgaria() {
        assert_eq!(timezone_to_alpha2("Europe/Sofia"), Some("BG"));
    }

    #[test]
    fn timezone_new_york_is_us() {
        assert_eq!(timezone_to_alpha2("America/New_York"), Some("US"));
    }

    #[test]
    fn timezone_seoul_is_korea() {
        assert_eq!(timezone_to_alpha2("Asia/Seoul"), Some("KR"));
    }

    #[test]
    fn timezone_berlin_is_germany() {
        assert_eq!(timezone_to_alpha2("Europe/Berlin"), Some("DE"));
    }

    #[test]
    fn timezone_shanghai_is_china() {
        assert_eq!(timezone_to_alpha2("Asia/Shanghai"), Some("CN"));
    }

    #[test]
    fn timezone_taipei_is_taiwan() {
        assert_eq!(timezone_to_alpha2("Asia/Taipei"), Some("TW"));
    }

    #[test]
    fn timezone_tokyo_is_japan() {
        assert_eq!(timezone_to_alpha2("Asia/Tokyo"), Some("JP"));
    }

    #[test]
    fn timezone_sydney_is_australia() {
        assert_eq!(timezone_to_alpha2("Australia/Sydney"), Some("AU"));
    }

    #[test]
    fn timezone_bangkok_is_thailand() {
        assert_eq!(timezone_to_alpha2("Asia/Bangkok"), Some("TH"));
    }

    #[test]
    fn timezone_utc_returns_none() {
        assert_eq!(timezone_to_alpha2("UTC"), None);
    }

    #[test]
    fn timezone_etc_gmt_returns_none() {
        assert_eq!(timezone_to_alpha2("Etc/GMT"), None);
    }

    #[test]
    fn timezone_unknown_returns_none() {
        assert_eq!(timezone_to_alpha2("Fake/Timezone"), None);
    }

    #[test]
    fn timezone_legacy_alias_calcutta() {
        assert_eq!(timezone_to_alpha2("Asia/Calcutta"), Some("IN"));
        assert_eq!(timezone_to_alpha2("Asia/Kolkata"), Some("IN"));
    }

    #[test]
    fn timezone_legacy_alias_kiev() {
        assert_eq!(timezone_to_alpha2("Europe/Kiev"), Some("UA"));
        assert_eq!(timezone_to_alpha2("Europe/Kyiv"), Some("UA"));
    }

    // --- country_to_blizzard_region ---

    #[test]
    fn region_us_countries() {
        assert_eq!(country_to_blizzard_region("US"), "US");
        assert_eq!(country_to_blizzard_region("CA"), "US");
        assert_eq!(country_to_blizzard_region("BR"), "US");
        assert_eq!(country_to_blizzard_region("MX"), "US");
        assert_eq!(country_to_blizzard_region("AR"), "US");
    }

    #[test]
    fn region_eu_countries() {
        assert_eq!(country_to_blizzard_region("DE"), "EU");
        assert_eq!(country_to_blizzard_region("FR"), "EU");
        assert_eq!(country_to_blizzard_region("GB"), "EU");
        assert_eq!(country_to_blizzard_region("BG"), "EU");
        assert_eq!(country_to_blizzard_region("RU"), "EU");
        assert_eq!(country_to_blizzard_region("ZA"), "EU");
        assert_eq!(country_to_blizzard_region("IN"), "EU");
        assert_eq!(country_to_blizzard_region("PK"), "EU");
        assert_eq!(country_to_blizzard_region("TR"), "EU");
    }

    #[test]
    fn region_dedicated() {
        assert_eq!(country_to_blizzard_region("KR"), "KR");
        assert_eq!(country_to_blizzard_region("TW"), "TW");
        assert_eq!(country_to_blizzard_region("CN"), "CN");
    }

    #[test]
    fn region_sg_countries() {
        assert_eq!(country_to_blizzard_region("AU"), "SG");
        assert_eq!(country_to_blizzard_region("NZ"), "SG");
        assert_eq!(country_to_blizzard_region("JP"), "SG");
        assert_eq!(country_to_blizzard_region("SG"), "SG");
        assert_eq!(country_to_blizzard_region("TH"), "SG");
        assert_eq!(country_to_blizzard_region("PH"), "SG");
        assert_eq!(country_to_blizzard_region("MY"), "SG");
        assert_eq!(country_to_blizzard_region("VN"), "SG");
        assert_eq!(country_to_blizzard_region("ID"), "SG");
        assert_eq!(country_to_blizzard_region("HK"), "SG");
    }

    #[test]
    fn region_unknown_defaults_to_us() {
        assert_eq!(country_to_blizzard_region("XX"), "US");
        assert_eq!(country_to_blizzard_region("ZZ"), "US");
    }

    // --- detect_geo_defaults_from_timezone ---

    #[test]
    fn detect_bulgaria() {
        let geo = detect_geo_defaults_from_timezone("Europe/Sofia").unwrap();
        assert_eq!(geo.alpha2, "BG");
        assert_eq!(geo.alpha3, "BGR");
        assert_eq!(geo.blizzard_region, "EU");
    }

    #[test]
    fn detect_germany() {
        let geo = detect_geo_defaults_from_timezone("Europe/Berlin").unwrap();
        assert_eq!(geo.alpha2, "DE");
        assert_eq!(geo.alpha3, "DEU");
        assert_eq!(geo.blizzard_region, "EU");
    }

    #[test]
    fn detect_us_east() {
        let geo = detect_geo_defaults_from_timezone("America/New_York").unwrap();
        assert_eq!(geo.alpha2, "US");
        assert_eq!(geo.alpha3, "USA");
        assert_eq!(geo.blizzard_region, "US");
    }

    #[test]
    fn detect_south_korea() {
        let geo = detect_geo_defaults_from_timezone("Asia/Seoul").unwrap();
        assert_eq!(geo.alpha2, "KR");
        assert_eq!(geo.alpha3, "KOR");
        assert_eq!(geo.blizzard_region, "KR");
    }

    #[test]
    fn detect_taiwan() {
        let geo = detect_geo_defaults_from_timezone("Asia/Taipei").unwrap();
        assert_eq!(geo.alpha2, "TW");
        assert_eq!(geo.alpha3, "TWN");
        assert_eq!(geo.blizzard_region, "TW");
    }

    #[test]
    fn detect_china() {
        let geo = detect_geo_defaults_from_timezone("Asia/Shanghai").unwrap();
        assert_eq!(geo.alpha2, "CN");
        assert_eq!(geo.alpha3, "CHN");
        assert_eq!(geo.blizzard_region, "CN");
    }

    #[test]
    fn detect_australia() {
        let geo = detect_geo_defaults_from_timezone("Australia/Sydney").unwrap();
        assert_eq!(geo.alpha2, "AU");
        assert_eq!(geo.alpha3, "AUS");
        assert_eq!(geo.blizzard_region, "SG");
    }

    #[test]
    fn detect_thailand() {
        let geo = detect_geo_defaults_from_timezone("Asia/Bangkok").unwrap();
        assert_eq!(geo.alpha2, "TH");
        assert_eq!(geo.alpha3, "THA");
        assert_eq!(geo.blizzard_region, "SG");
    }

    #[test]
    fn detect_utc_returns_none() {
        assert!(detect_geo_defaults_from_timezone("UTC").is_none());
    }

    #[test]
    fn detect_unknown_returns_none() {
        assert!(detect_geo_defaults_from_timezone("Fake/Nowhere").is_none());
    }

    // --- Validates real .build.info patterns from test installations ---

    #[test]
    fn matches_real_build_info_bulgarian_install() {
        // 1.13.2.31650: acct-BGR geoip-BG, region EU
        let geo = detect_geo_defaults_from_timezone("Europe/Sofia").unwrap();
        assert_eq!(geo.alpha3, "BGR"); // acct-BGR
        assert_eq!(geo.alpha2, "BG"); // geoip-BG
        assert_eq!(geo.blizzard_region, "EU"); // EU?
    }

    #[test]
    fn matches_real_build_info_german_install() {
        // 1.15.2.55140: acct-DEU geoip-DE, region EU
        let geo = detect_geo_defaults_from_timezone("Europe/Berlin").unwrap();
        assert_eq!(geo.alpha3, "DEU"); // acct-DEU
        assert_eq!(geo.alpha2, "DE"); // geoip-DE
        assert_eq!(geo.blizzard_region, "EU"); // EU?
    }
}
