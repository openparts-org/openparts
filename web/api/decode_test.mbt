// Decodes real API responses (RP2040, the USB-C package) captured from a
// local openparts-server with the generated types.

///|
test "decode Part 0" {
  let text =
    #|{
    #| "schema_version": "0.1",
    #| "kind": "part",
    #| "id": "raspberrypi/RP2040",
    #| "manufacturer": "raspberrypi",
    #| "mpn": "RP2040",
    #| "device": "raspberrypi/RP2040",
    #| "package": "standards/QFN56-RP2040",
    #| "existence": {
    #|  "status": "verified"
    #| },
    #| "lifecycle": {
    #|  "status": "active",
    #|  "replacement": []
    #| },
    #| "sources": [
    #|  "raspberrypi/RP-008371-DS@rev1"
    #| ],
    #| "provenance": {}
    #|}
  let json = @json.parse(text)
  let _ : @api.Part = @json.from_json(json)
}

///|
test "decode Device 1" {
  let text =
    #|{
    #| "schema_version": "0.1",
    #| "kind": "device",
    #| "id": "raspberrypi/RP2040",
    #| "manufacturer": "raspberrypi",
    #| "category": "microcontroller",
    #| "family": "RP2040",
    #| "pins": {
    #|  "1": {
    #|   "name": "IOVDD",
    #|   "type": "power",
    #|   "alternate_functions": []
    #|  },
    #|  "10": {
    #|   "name": "IOVDD",
    #|   "type": "power",
    #|   "alternate_functions": []
    #|  },
    #|  "11": {
    #|   "name": "GPIO8",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "12": {
    #|   "name": "GPIO9",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "13": {
    #|   "name": "GPIO10",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "14": {
    #|   "name": "GPIO11",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "15": {
    #|   "name": "GPIO12",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "16": {
    #|   "name": "GPIO13",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "17": {
    #|   "name": "GPIO14",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "18": {
    #|   "name": "GPIO15",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "19": {
    #|   "name": "TESTEN",
    #|   "type": "reserved",
    #|   "alternate_functions": []
    #|  },
    #|  "2": {
    #|   "name": "GPIO0",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "20": {
    #|   "name": "XIN",
    #|   "type": "clock",
    #|   "alternate_functions": []
    #|  },
    #|  "21": {
    #|   "name": "XOUT",
    #|   "type": "clock",
    #|   "alternate_functions": []
    #|  },
    #|  "22": {
    #|   "name": "IOVDD",
    #|   "type": "power",
    #|   "alternate_functions": []
    #|  },
    #|  "23": {
    #|   "name": "DVDD",
    #|   "type": "power",
    #|   "alternate_functions": []
    #|  },
    #|  "24": {
    #|   "name": "SWCLK",
    #|   "type": "clock",
    #|   "alternate_functions": []
    #|  },
    #|  "25": {
    #|   "name": "SWD",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "26": {
    #|   "name": "RUN",
    #|   "type": "reset",
    #|   "alternate_functions": []
    #|  },
    #|  "27": {
    #|   "name": "GPIO16",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "28": {
    #|   "name": "GPIO17",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "29": {
    #|   "name": "GPIO18",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "3": {
    #|   "name": "GPIO1",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "30": {
    #|   "name": "GPIO19",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "31": {
    #|   "name": "GPIO20",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "32": {
    #|   "name": "GPIO21",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "33": {
    #|   "name": "IOVDD",
    #|   "type": "power",
    #|   "alternate_functions": []
    #|  },
    #|  "34": {
    #|   "name": "GPIO22",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "35": {
    #|   "name": "GPIO23",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "36": {
    #|   "name": "GPIO24",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "37": {
    #|   "name": "GPIO25",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "38": {
    #|   "name": "GPIO26",
    #|   "type": "io",
    #|   "alternate_functions": [
    #|    "ADC0"
    #|   ]
    #|  },
    #|  "39": {
    #|   "name": "GPIO27",
    #|   "type": "io",
    #|   "alternate_functions": [
    #|    "ADC1"
    #|   ]
    #|  },
    #|  "4": {
    #|   "name": "GPIO2",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "40": {
    #|   "name": "GPIO28",
    #|   "type": "io",
    #|   "alternate_functions": [
    #|    "ADC2"
    #|   ]
    #|  },
    #|  "41": {
    #|   "name": "GPIO29",
    #|   "type": "io",
    #|   "alternate_functions": [
    #|    "ADC3"
    #|   ]
    #|  },
    #|  "42": {
    #|   "name": "IOVDD",
    #|   "type": "power",
    #|   "alternate_functions": []
    #|  },
    #|  "43": {
    #|   "name": "ADC_AVDD",
    #|   "type": "power",
    #|   "alternate_functions": []
    #|  },
    #|  "44": {
    #|   "name": "VREG_VIN",
    #|   "type": "power",
    #|   "alternate_functions": []
    #|  },
    #|  "45": {
    #|   "name": "VREG_VOUT",
    #|   "type": "power_out",
    #|   "alternate_functions": []
    #|  },
    #|  "46": {
    #|   "name": "USB_DM",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "47": {
    #|   "name": "USB_DP",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "48": {
    #|   "name": "USB_VDD",
    #|   "type": "power",
    #|   "alternate_functions": []
    #|  },
    #|  "49": {
    #|   "name": "IOVDD",
    #|   "type": "power",
    #|   "alternate_functions": []
    #|  },
    #|  "5": {
    #|   "name": "GPIO3",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "50": {
    #|   "name": "DVDD",
    #|   "type": "power",
    #|   "alternate_functions": []
    #|  },
    #|  "51": {
    #|   "name": "QSPI_SD3",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "52": {
    #|   "name": "QSPI_SCLK",
    #|   "type": "clock_out",
    #|   "alternate_functions": []
    #|  },
    #|  "53": {
    #|   "name": "QSPI_SD0",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "54": {
    #|   "name": "QSPI_SD2",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "55": {
    #|   "name": "QSPI_SD1",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "56": {
    #|   "name": "QSPI_CSn",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "6": {
    #|   "name": "GPIO4",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "7": {
    #|   "name": "GPIO5",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "8": {
    #|   "name": "GPIO6",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "9": {
    #|   "name": "GPIO7",
    #|   "type": "io",
    #|   "alternate_functions": []
    #|  },
    #|  "EP": {
    #|   "name": "GND",
    #|   "type": "ground",
    #|   "alternate_functions": []
    #|  }
    #| },
    #| "revisions": {
    #|  "rev-b0": {
    #|   "manufacturer_revision": "B0",
    #|   "detection": [
    #|    {
    #|     "method": "device_register",
    #|     "register": "CHIP_ID",
    #|     "mask": null,
    #|     "value": null
    #|    }
    #|   ],
    #|   "overrides": {
    #|    "pins": {}
    #|   }
    #|  },
    #|  "rev-b1": {
    #|   "manufacturer_revision": "B1",
    #|   "detection": [
    #|    {
    #|     "method": "device_register",
    #|     "register": "CHIP_ID",
    #|     "mask": null,
    #|     "value": null
    #|    }
    #|   ],
    #|   "overrides": {
    #|    "pins": {}
    #|   }
    #|  },
    #|  "rev-b2": {
    #|   "manufacturer_revision": "B2",
    #|   "detection": [
    #|    {
    #|     "method": "device_register",
    #|     "register": "CHIP_ID",
    #|     "mask": null,
    #|     "value": null
    #|    }
    #|   ],
    #|   "overrides": {
    #|    "pins": {}
    #|   }
    #|  }
    #| },
    #| "provenance": {
    #|  "/pins/1/name": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": null,
    #|    "locator": "Chapter 5.5.2, Table615-621"
    #|   }
    #|  ],
    #|  "/pins/EP/name": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": null,
    #|    "locator": "\"Common ground connection via central pad\""
    #|   }
    #|  ],
    #|  "/revisions": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": 130,
    #|    "locator": null
    #|   }
    #|  ]
    #| }
    #|}
  let json = @json.parse(text)
  let _ : @api.Device = @json.from_json(json)
}

///|
test "decode Package 2" {
  let text =
    #|{
    #| "schema_version": "0.1",
    #| "kind": "package",
    #| "id": "standards/QFN56-RP2040",
    #| "family": "qfn",
    #| "lead_count": 56,
    #| "pitch": {
    #|  "nominal": 0.4,
    #|  "min": null,
    #|  "max": null,
    #|  "unit": "mm"
    #| },
    #| "dimensions": {
    #|  "body_width": {
    #|   "nominal": 7.0,
    #|   "min": null,
    #|   "max": null,
    #|   "unit": "mm"
    #|  },
    #|  "body_length": {
    #|   "nominal": 7.0,
    #|   "min": null,
    #|   "max": null,
    #|   "unit": "mm"
    #|  },
    #|  "body_height": null,
    #|  "exposed_pad": {
    #|   "width": {
    #|    "nominal": 3.1,
    #|    "min": 3.0,
    #|    "max": 3.2,
    #|    "unit": "mm"
    #|   },
    #|   "length": {
    #|    "nominal": 3.1,
    #|    "min": 3.0,
    #|    "max": 3.2,
    #|    "unit": "mm"
    #|   }
    #|  },
    #|  "standoff": {
    #|   "nominal": null,
    #|   "min": 0.0,
    #|   "max": 0.05,
    #|   "unit": "mm"
    #|  },
    #|  "lead_width": {
    #|   "nominal": 0.18,
    #|   "min": 0.13,
    #|   "max": 0.23,
    #|   "unit": "mm"
    #|  },
    #|  "lead_length": {
    #|   "nominal": 0.4,
    #|   "min": 0.3,
    #|   "max": 0.5,
    #|   "unit": "mm"
    #|  },
    #|  "lead_thickness": {
    #|   "nominal": 0.203,
    #|   "min": null,
    #|   "max": null,
    #|   "unit": "mm"
    #|  }
    #| },
    #| "lead_layout": null,
    #| "geometry": {
    #|  "type": "parametric",
    #|  "generator": "qfn"
    #| },
    #| "provenance": {
    #|  "/dimensions/body_length": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": null,
    #|    "locator": "\"QFN-56 7x7mm (reduced ePad size)\""
    #|   }
    #|  ],
    #|  "/dimensions/body_width": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": null,
    #|    "locator": "\"QFN-56 7x7mm (reduced ePad size)\""
    #|   }
    #|  ],
    #|  "/dimensions/exposed_pad/length": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": 608,
    #|    "locator": "Section 5.1 Package, Figure 166 dimension table, symbol \"E2\" (3.00/3.100/3.200)"
    #|   }
    #|  ],
    #|  "/dimensions/exposed_pad/width": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": 608,
    #|    "locator": "Section 5.1 Package, Figure 166 dimension table, symbol \"D2\" (3.00/3.100/3.200; drawn along X in the bottom view)"
    #|   }
    #|  ],
    #|  "/dimensions/lead_length": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": 608,
    #|    "locator": "Section 5.1 Package, Figure 166 dimension table, symbol \"L\""
    #|   }
    #|  ],
    #|  "/dimensions/lead_thickness": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": 608,
    #|    "locator": "Section5.1 Package, Figure 166 dimension table, symbol \"A3\" (0.203 REF)"
    #|   }
    #|  ],
    #|  "/dimensions/lead_width": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": 608,
    #|    "locator": "Section 5.1 Package, Figure 166 dimension table, symbol \"b\""
    #|   }
    #|  ],
    #|  "/dimensions/standoff": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": 608,
    #|    "locator": "Section 5.1 Package, Figure 166 dimension table, symbol \"A1\" (no nom)"
    #|   }
    #|  ],
    #|  "/lead_count": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": null,
    #|    "locator": "Section 5.5.2 package description"
    #|   }
    #|  ],
    #|  "/pitch": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": null,
    #|    "locator": "Section 5.5.2, \"standard 0.4mm QFN-56 footprints\""
    #|   }
    #|  ]
    #| }
    #|}
  let json = @json.parse(text)
  let _ : @api.Package = @json.from_json(json)
}

///|
test "decode Array[PartSource] 3" {
  let text =
    #|[
    #| {
    #|  "source": {
    #|   "schema_version": "0.1",
    #|   "kind": "source",
    #|   "id": "raspberrypi/RP-008371-DS@rev1",
    #|   "publisher": "raspberrypi",
    #|   "document_id": "RP-008371-DS",
    #|   "document_type": "datasheet",
    #|   "revision": "1",
    #|   "date": "2025-02-20",
    #|   "url": "https://datasheets.raspberrypi.com/rp2040/rp2040-datasheet.pdf"
    #|  },
    #|  "cited_by": [
    #|   "part",
    #|   "device",
    #|   "package"
    #|  ]
    #| }
    #|]
  let json = @json.parse(text)
  let _ : Array[@api.PartSource] = @json.from_json(json)
}

///|
test "decode Array[RevisionSummary] 4" {
  let text =
    #|[
    #| {
    #|  "id": "rev-b0",
    #|  "manufacturer_revision": "B0"
    #| },
    #| {
    #|  "id": "rev-b1",
    #|  "manufacturer_revision": "B1"
    #| },
    #| {
    #|  "id": "rev-b2",
    #|  "manufacturer_revision": "B2"
    #| }
    #|]
  let json = @json.parse(text)
  let _ : Array[@api.RevisionSummary] = @json.from_json(json)
}

///|
test "decode ProvenanceResponse 5" {
  let text =
    #|{
    #| "part": {},
    #| "device": {
    #|  "/pins/1/name": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": null,
    #|    "locator": "Chapter 5.5.2, Table 615-621"
    #|   }
    #|  ],
    #|  "/pins/EP/name": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": null,
    #|    "locator": "\"Common ground connection via central pad\""
    #|   }
    #|  ],
    #|  "/revisions": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": 130,
    #|    "locator": null
    #|   }
    #|  ]
    #| },
    #| "package": {
    #|  "/dimensions/body_length": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": null,
    #|    "locator": "\"QFN-56 7x7mm (reduced ePad size)\""
    #|   }
    #|  ],
    #|  "/dimensions/body_width": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": null,
    #|    "locator": "\"QFN-56 7x7mm (reduced ePad size)\""
    #|   }
    #|  ],
    #|  "/dimensions/exposed_pad/length": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": 608,
    #|    "locator": "Section 5.1 Package, Figure 166 dimension table, symbol \"E2\" (3.00/3.100/3.200)"
    #|   }
    #|  ],
    #|  "/dimensions/exposed_pad/width": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": 608,
    #|    "locator": "Section 5.1 Package, Figure 166 dimension table, symbol \"D2\" (3.00/3.100/3.200; drawn along X in the bottom view)"
    #|   }
    #|  ],
    #|  "/dimensions/lead_length": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": 608,
    #|    "locator": "Section 5.1 Package, Figure 166 dimension table, symbol \"L\""
    #|   }
    #|  ],
    #|  "/dimensions/lead_thickness": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": 608,
    #|    "locator": "Section 5.1 Package, Figure 166 dimension table, symbol \"A3\" (0.203 REF)"
    #|   }
    #|  ],
    #|  "/dimensions/lead_width": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": 608,
    #|    "locator": "Section 5.1 Package, Figure 166 dimension table, symbol \"b\""
    #|   }
    #|  ],
    #|  "/dimensions/standoff": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": 608,
    #|    "locator": "Section 5.1 Package, Figure 166 dimension table, symbol \"A1\" (no nom)"
    #|   }
    #|  ],
    #|  "/lead_count": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": null,
    #|    "locator": "Section 5.5.2 package description"
    #|   }
    #|  ],
    #|  "/pitch": [
    #|   {
    #|    "source": "raspberrypi/RP-008371-DS@rev1",
    #|    "page": null,
    #|    "locator": "Section 5.5.2, \"standard 0.4mm QFN-56 footprints\""
    #|   }
    #|  ]
    #| }
    #|}
  let json = @json.parse(text)
  let _ : @api.ProvenanceResponse = @json.from_json(json)
}

///|
test "decode ArtifactsResponse 6" {
  let text =
    #|{
    #| "kicad-symbol": "/v1/parts/raspberrypi/RP2040/artifacts/kicad-symbol",
    #| "kicad-footprint": "/v1/parts/raspberrypi/RP2040/artifacts/kicad-footprint",
    #| "footprint-svg": "/v1/parts/raspberrypi/RP2040/artifacts/footprint-svg",
    #| "step": "/v1/parts/raspberrypi/RP2040/artifacts/step",
    #| "stl": "/v1/parts/raspberrypi/RP2040/artifacts/stl",
    #| "3mf": "/v1/parts/raspberrypi/RP2040/artifacts/3mf"
    #|}
  let json = @json.parse(text)
  let _ : @api.ArtifactsResponse = @json.from_json(json)
}

///|
test "decode Array[PartSummary] 7" {
  let text =
    #|[
    #| {
    #|  "manufacturer": "onsemi",
    #|  "mpn": "NCP1117DT33RKG",
    #|  "existence": "verified",
    #|  "lifecycle": "unknown",
    #|  "category": "other"
    #| },
    #| {
    #|  "manufacturer": "onsemi",
    #|  "mpn": "NCP1117DT50RKG",
    #|  "existence": "verified",
    #|  "lifecycle": "unknown",
    #|  "category": "other"
    #| },
    #| {
    #|  "manufacturer": "onsemi",
    #|  "mpn": "NCP1117ST33T3G",
    #|  "existence": "verified",
    #|  "lifecycle": "unknown",
    #|  "category": "other"
    #| },
    #| {
    #|  "manufacturer": "onsemi",
    #|  "mpn": "NCP1117ST50T3G",
    #|  "existence": "verified",
    #|  "lifecycle": "unknown",
    #|  "category": "other"
    #| }
    #|]
  let json = @json.parse(text)
  let _ : Array[@api.PartSummary] = @json.from_json(json)
}

///|
test "decode Package 8" {
  let text =
    #|{
    #| "schema_version": "0.1",
    #| "kind": "package",
    #| "id": "manufacturers/gct/USB4105",
    #| "family": "usb_c",
    #| "lead_count": 16,
    #| "pitch": {
    #|  "nominal": null,
    #|  "min": null,
    #|  "max": null,
    #|  "unit": "mm"
    #| },
    #| "dimensions": {
    #|  "body_width": {
    #|   "nominal": 8.94,
    #|   "min": null,
    #|   "max": null,
    #|   "unit": "mm"
    #|  },
    #|  "body_length": {
    #|   "nominal": 7.35,
    #|   "min": null,
    #|   "max": null,
    #|   "unit": "mm"
    #|  },
    #|  "body_height": {
    #|   "nominal": 3.31,
    #|   "min": null,
    #|   "max": null,
    #|   "unit": "mm"
    #|  },
    #|  "exposed_pad": null,
    #|  "body_offset": {
    #|   "nominal": -1.075,
    #|   "min": null,
    #|   "max": null,
    #|   "unit": "mm"
    #|  }
    #| },
    #| "lead_layout": null,
    #| "pads": [
    #|  {
    #|   "number": "A1",
    #|   "x": 3.2,
    #|   "y": -4.755,
    #|   "width": 0.6,
    #|   "length": 1.15
    #|  },
    #|  {
    #|   "number": "A4",
    #|   "x": 2.4,
    #|   "y": -4.755,
    #|   "width": 0.6,
    #|   "length": 1.15
    #|  },
    #|  {
    #|   "number": "B8",
    #|   "x": 1.75,
    #|   "y": -4.755,
    #|   "width": 0.3,
    #|   "length": 1.15
    #|  },
    #|  {
    #|   "number": "A5",
    #|   "x": 1.25,
    #|   "y": -4.755,
    #|   "width": 0.3,
    #|   "length": 1.15
    #|  },
    #|  {
    #|   "number": "B7",
    #|   "x": 0.75,
    #|   "y": -4.755,
    #|   "width": 0.3,
    #|   "length": 1.15
    #|  },
    #|  {
    #|   "number": "A6",
    #|   "x": 0.25,
    #|   "y": -4.755,
    #|   "width": 0.3,
    #|   "length": 1.15
    #|  },
    #|  {
    #|   "number": "A7",
    #|   "x": -0.25,
    #|   "y": -4.755,
    #|   "width": 0.3,
    #|   "length": 1.15
    #|  },
    #|  {
    #|   "number": "B6",
    #|   "x": -0.75,
    #|   "y": -4.755,
    #|   "width": 0.3,
    #|   "length": 1.15
    #|  },
    #|  {
    #|   "number": "A8",
    #|   "x": -1.25,
    #|   "y": -4.755,
    #|   "width": 0.3,
    #|   "length": 1.15
    #|  },
    #|  {
    #|   "number": "B5",
    #|   "x": -1.75,
    #|   "y": -4.755,
    #|   "width": 0.3,
    #|   "length": 1.15
    #|  },
    #|  {
    #|   "number": "A9",
    #|   "x": -2.4,
    #|   "y": -4.755,
    #|   "width": 0.6,
    #|   "length": 1.15
    #|  },
    #|  {
    #|   "number": "A12",
    #|   "x": -3.2,
    #|   "y": -4.755,
    #|   "width": 0.6,
    #|   "length": 1.15
    #|  },
    #|  {
    #|   "number": "SHELL",
    #|   "x": 4.32,
    #|   "y": -4.18,
    #|   "width": 1.0,
    #|   "length": 2.1,
    #|   "drill": 0.6,
    #|   "drill_length": 1.7
    #|  },
    #|  {
    #|   "number": "SHELL",
    #|   "x": -4.32,
    #|   "y": -4.18,
    #|   "width": 1.0,
    #|   "length": 2.1,
    #|   "drill": 0.6,
    #|   "drill_length": 1.7
    #|  },
    #|  {
    #|   "number": "SHELL",
    #|   "x": 4.32,
    #|   "y": 0.0,
    #|   "width": 1.0,
    #|   "length": 1.8,
    #|   "drill": 0.6,
    #|   "drill_length": 1.4
    #|  },
    #|  {
    #|   "number": "SHELL",
    #|   "x": -4.32,
    #|   "y": 0.0,
    #|   "width": 1.0,
    #|   "length": 1.8,
    #|   "drill": 0.6,
    #|   "drill_length": 1.4
    #|  }
    #| ],
    #| "geometry": {
    #|  "type": "parametric",
    #|  "generator": "usb_c"
    #| },
    #| "provenance": {
    #|  "/dimensions/body_height": [
    #|   {
    #|    "source": "gct/USB4105-DWG",
    #|    "page": 1,
    #|    "locator": "Side view, seating plane to shell top \"3.31\""
    #|   }
    #|  ],
    #|  "/dimensions/body_length": [
    #|   {
    #|    "source": "gct/USB4105-DWG",
    #|    "page": 1,
    #|    "locator": "Top view, shell length \"7.35\""
    #|   }
    #|  ],
    #|  "/dimensions/body_offset": [
    #|   {
    #|    "source": "gct/USB4105-DWG",
    #|    "page": 1,
    #|    "locator": "Side view: front face to front tab centre \"2.60\", tab to tab \"4.18\"; shell length \"7.35\": centre y = 2.60 - 7.35/2 = -1.075 (opening toward +Y)"
    #|   }
    #|  ],
    #|  "/dimensions/body_width": [
    #|   {
    #|    "source": "gct/USB4105-DWG",
    #|    "page": 1,
    #|    "locator": "Top view (left of the PCB layout), shell width \"8.94\""
    #|   }
    #|  ],
    #|  "/lead_count": [
    #|   {
    #|    "source": "gct/USB4105-DWG",
    #|    "page": 1,
    #|    "locator": "\"Recommended PCB Layout\": 12 contact pads (A1 B12, A4 B9, B8, A5, B7, A6, A7, B6, A8, B5, B4 A9, B1 A12) + 4 plated shell slots"
    #|   }
    #|  ],
    #|  "/pads": [
    #|   {
    #|    "source": "gct/USB4105-DWG",
    #|    "page": 1,
    #|    "locator": "\"Recommended PCB Layout\" (as viewed from component side, tolerance ±0.05), drawn with the PCB edge (opening) at the bottom; turned 180° so the opening faces +Y (x -> -x, y -> -y; rotation, not mirror). Origin: X centre line, Y front (PCB-edge side) shell slot centres. X (drawing, left to right A1/B12, A4/B9, B8, A5, B7, A6, A7, B6, A8, B5,B4/A9, B1/A12): \"6.40\" outer pad centres -> ±3.2; \"4.80\" A4/B9 to B4/A9 -> ±2.4; \"3.50\" B8 to B5 -> ±1.75; pitch \"0.50\" in between (A5 ±1.25, B7 ±0.75, A6/A7 ±0.25); then x -> -x by the rotation. Widths \"4x 0.60\" (double-labelledpads), \"8x 0.30\"; length \"12x 1.15\". Y: \"12x 4.18\" from the front slot centres to the signal pads' rear-of-opening edge (the bottom edge in the drawing), so centre y = -(4.18 + 1.15/2) = -4.755. Shell slots (pin \"SHELL\"): \"8.64\" between centres -> x ±4.32; rear slot centres on that same 4.18 line (side view: tab to tab \"4.18\") -> y -4.18, land \"2x 2.10\" x \"4x 1.00\", slot \"2x 1.70\" x \"4x 0.60\"; front slots y 0, land \"2x 1.80\" x 1.00, slot \"2x 1.40\" x 0.60; \"2x 2.60\" front slots to PCB edge. Not recorded:the two non-plated \"2x Ø0.65\" alignment holes (\"5.78\" apart, \"2x 3.68\" from the front slots) -- the family has no non-plated hole."
    #|   }
    #|  ]
    #| }
    #|}
  let json = @json.parse(text)
  let _ : @api.Package = @json.from_json(json)
}
