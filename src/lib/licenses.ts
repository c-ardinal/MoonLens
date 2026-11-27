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
  /** 契約名。 */
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
    title: 'PLaMo コミュニティライセンス契約',
    lead: 'PLaMo 3 Translate の取得・利用には PLaMo コミュニティライセンス契約への同意が必要。以下は要点。正式な条件は契約全文を参照。',
    points: [
      '私的利用・非商用利用（学術用途を包含）：無償、登録不要',
      '商用利用（組織内の業務利用を包含）：事前登録が必要。対象は直近事業年度の収入・売上 10 億円以下',
      '収入・売上 10 億円超の商用利用：開発元との個別契約が必要',
      '利用者は 18 歳以上（未満は親権者の同意が必要）',
      '出力は無保証。重要事項の決定で唯一の根拠とする利用は禁止'
    ],
    fullText: { label: '契約全文', href: 'https://plamo.preferredai.jp/info/plamo-community-license-ja' },
    links: [
      { label: '商用利用の登録フォーム', href: 'https://forms.gle/mTL8tBLrMYXKNZD56' },
      { label: '個別契約の問合せ', href: 'https://translate.preferredai.jp/llm/contact/' }
    ]
  }
};
