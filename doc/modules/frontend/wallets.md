# 钱包账户

最后更新：2026-07-15

`/wallets` 管理多钱包执行账户。前端只提交 `credential_provider + credential_locator + key_version`，不接受或保存明文私钥。

关键文件：`src/app/(console)/wallets/page.tsx`、`src/features/wallets/components/wallets-workbench.tsx`、`src/lib/api/wallets.ts`、`src/lib/api/actions/wallets.ts`、`src/lib/contracts/dto/wallets.ts`。

创建请求完整覆盖 signer/funder、signature type、凭证定位符、交易开关和五项钱包风控上限。列表按后端 `WalletAccountData` 展示 account、credential、risk_policy 与 account state，包括可用抵押品、开放买入名义金额和最近同步错误。默认关闭交易；显式勾选启用时必须输入 step-up code，并以 `wallet_trading_enable` scope 提交。
