// Pre-filled GitHub "new issue" links pointing at openparts-data -- that
// repo owns the canonical data, so data-quality reports belong there, not
// against this site's own repo. No labels are passed: openparts-data does
// not (yet) define any issue-template labels to target.

const OPENPARTS_DATA_ISSUES_URL =
  "https://github.com/openparts-org/openparts-data/issues/new";

export function reportDataIssueUrl(params: {
  manufacturer: string;
  mpn: string;
  deviceId: string;
  pageUrl: string;
}): string {
  const title = `Data issue: ${params.manufacturer}/${params.mpn}`;
  const body = [
    `**Part**: ${params.manufacturer}/${params.mpn}`,
    `**Device**: ${params.deviceId}`,
    `**Page**: ${params.pageUrl}`,
    "",
    "Describe the problem (e.g. incorrect pin number, name, or type):",
    "",
  ].join("\n");

  const url = new URL(OPENPARTS_DATA_ISSUES_URL);
  url.searchParams.set("title", title);
  url.searchParams.set("body", body);
  return url.toString();
}
