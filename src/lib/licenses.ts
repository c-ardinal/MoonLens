/**
 * 取得前に同意を求めるライセンスの表。キーは `CatalogModel.consent` の id
 * （src-tauri/src/providers/llm_models.rs）。UI は本表のみを参照し、
 * モデル名による分岐を持たない。
 */

export interface LicenseLink {
  label: string;
  href: string;
}

export interface LicenseInfo {
  /** 契約名。本表の文言は文言のキー ID（表示時に `t()` で取得）。 */
  title: string;
  /** 同意が必要な旨と、要点が要約である旨の案内。 */
  lead: string;
  /** 要点（要約。正式な条件は契約全文を優先）。 */
  points: string[];
  /** 契約全文（正本）。 */
  fullText: LicenseLink;
  /** 関連リンク。 */
  links: LicenseLink[];
}

export const LICENSES: Record<string, LicenseInfo> = {
  'plamo-community': {
    title: 'licenses:plamo_community_license_agreement',
    lead: 'licenses:downloading_and_using_plamo_3_translate',
    points: [
      'licenses:personal_and_non_commercial_use_including',
      'licenses:commercial_use_including_internal_business',
      'licenses:commercial_use_with_revenue_above_1_billion',
      'licenses:users_must_be_18_or_older_minors',
      'licenses:output_comes_with_no_warranty_using_it'
    ],
    fullText: { label: 'licenses:full_agreement', href: 'https://plamo.preferredai.jp/info/plamo-community-license-ja' },
    links: [
      { label: 'licenses:commercial_use_registration_form', href: 'https://forms.gle/mTL8tBLrMYXKNZD56' },
      { label: 'licenses:enquiries_about_a_separate_agreement', href: 'https://translate.preferredai.jp/llm/contact/' }
    ]
  }
};
