import { reportDataIssueUrl } from "../lib/github";
import type { Device } from "../lib/types";

export default function PinTable({
  device,
  manufacturer,
  mpn,
}: {
  device: Device;
  manufacturer: string;
  mpn: string;
}) {
  const pinEntries = Object.entries(device.pins);

  const issueUrl = reportDataIssueUrl({
    manufacturer,
    mpn,
    deviceId: device.id,
    pageUrl: typeof window !== "undefined" ? window.location.href : "",
  });

  return (
    <div>
      <div class="pin-table-header">
        <h3>ピン配置 ({device.id})</h3>
        <a
          class="button"
          href={issueUrl}
          target="_blank"
          rel="noopener noreferrer"
        >
          このデータの問題を報告
        </a>
      </div>
      <details class="pin-table-details">
        <summary>{pinEntries.length}ピンを表示</summary>
        <table>
          <thead>
            <tr>
              <th>ピン番号</th>
              <th>名前</th>
              <th>種別</th>
              <th>代替機能</th>
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
