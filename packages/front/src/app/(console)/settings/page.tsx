import { SettingsWorkbench } from "@/features/settings/components/settings-workbench";
import { getSettingsPageData } from "@/features/settings/loaders/settings-page-data";
import { getServerI18n } from "@/lib/i18n/server";

export default async function SettingsPage() {
  const i18n = getServerI18n();
  const [data, { dictionary, format }] = await Promise.all([getSettingsPageData(i18n), i18n]);

  return <SettingsWorkbench data={data} dictionary={dictionary} format={format} />;
}
