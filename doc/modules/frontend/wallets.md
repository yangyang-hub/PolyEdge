# 钱包账户

最后更新：2026-07-16

`/wallets` 管理用户自有执行账户。浏览器获取一次性 RSA-OAEP-SHA256 公钥，生成 AES-256-GCM 数据密钥加密私钥，再提交 `encrypted_secret`；明文私钥不写入持久存储。只读用户只能查看钱包状态。

关键文件：`src/app/(console)/wallets/page.tsx`、`src/features/wallets/components/wallets-workbench.tsx`、`src/lib/api/wallets.ts`、`src/lib/api/actions/wallets.ts`、`src/lib/contracts/dto/wallets.ts`。

创建请求覆盖 signer/funder、signature type、`encrypted_secret`、交易开关和五项钱包风控上限。后端解密后验证 private key 推导地址与 signer 一致，再写数据库 envelope。列表按 `WalletAccountData` 展示 account、secret metadata、risk policy 与 account state，包括可用抵押品、开放买入名义金额和最近同步错误，不返回 ciphertext。

当前表单只收集 private key，未提供可选 CLOB API key/secret/passphrase 输入；后端 payload 格式已支持这些字段。默认关闭交易；显式启用时页面仍要求填写旧 step-up code，但后端实际要求 recent-auth session。页面尚未提供钱包编辑、secret rotation 或管理员查看他人钱包的专用筛选 UX。
