type CsvCell = string | number | boolean | null | undefined;

function escapeCell(value: CsvCell): string {
  const text = value === null || value === undefined ? "" : String(value);
  return `"${text.replaceAll('"', '""')}"`;
}

/**
 * 把表格数据导出为 CSV 文件并触发浏览器下载。
 * 所有单元格统一用双引号包裹、内部双引号转义，兼容含逗号/换行的值。
 */
export function downloadCsv(filename: string, headers: string[], rows: CsvCell[][]): void {
  const csv = [headers, ...rows]
    .map((row) => row.map(escapeCell).join(","))
    .join("\n");
  const url = URL.createObjectURL(new Blob([csv], { type: "text/csv;charset=utf-8" }));
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
}
