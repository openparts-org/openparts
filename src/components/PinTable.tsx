import { reportDataIssueUrl } from "../lib/github";
import { t, type Lang } from "../lib/i18n";
import type { Device } from "../lib/types";

export default function PinTable({
  device,
  manufacturer,
  mpn,
  lang,
}: {
  device: Device;
  manufacturer: string;
  mpn: string;
  lang: Lang;
}) {
  const copy = t[lang].part;
  const pinEntries = Object.entries(device.pins).flatMap(([number, pin]) =>
    pin ? [[number, pin] as const] : [],
  );

  const issueUrl = reportDataIssueUrl({
    manufacturer,
    mpn,
    deviceId: device.id,
    pageUrl: typeof window !== "undefined" ? window.location.href : "",
  });

  return (
    <div>
      <div class="pin-table-header">
        <h3>{copy.pinsHeading(device.id)}</h3>
        <a
          class="button"
          href={issueUrl}
          target="_blank"
          rel="noopener noreferrer"
        >
          {copy.reportIssue}
        </a>
      </div>
      <details class="pin-table-details">
        <summary>{copy.pinsShow(pinEntries.length)}</summary>
        <table>
          <thead>
            <tr>
              <th>{copy.pinNumber}</th>
              <th>{copy.pinName}</th>
              <th>{copy.pinType}</th>
              <th>{copy.pinAltFunctions}</th>
            </tr>
          </thead>
          <tbody>
            {pinEntries.map(([number, pin]) => (
              <tr key={number}>
                <td>{number}</td>
                <td>{pin.name}</td>
                <td>{pin.type}</td>
                <td>{pin.alternate_functions.join(", ") || "-"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </div>
  );
}
