import { RPC_VERSION } from "@alcomd/sdk";
import { productFamily, technicalName } from "@alcomd/ui";
import { createElement } from "react";

export function App() {
    const materialButton = createElement(
        "md-filled-button",
        {
            onClick: () => {
                window.alert("当前只是 M0 仓库骨架，尚未连接 alcomd。");
            }
        },
        "检查骨架状态"
    );

    return (
        <main className="shell">
            <section className="hero">
                <p className="eyebrow">{productFamily} · v4 仓库骨架</p>
                <h1>ALCOMD3</h1>
                <p className="subtitle">
                    用户品牌是 ALCOMD3，产品家族是 {productFamily}，稳定技术平台是
                    <code>{technicalName}</code>。
                </p>
                <div className="status-grid" aria-label="初始化状态">
                    <article>
                        <span>RPC</span>
                        <strong>v{RPC_VERSION} 骨架</strong>
                    </article>
                    <article>
                        <span>核心</span>
                        <strong>尚未连接</strong>
                    </article>
                    <article>
                        <span>阶段</span>
                        <strong>M0 骨架收敛</strong>
                    </article>
                </div>
                <div className="actions">{materialButton}</div>
            </section>

            <section className="notice">
                <h2>业务实现尚未开始</h2>
                <p>
                    M-1 审计与合同基线已经冻结。当前仅验证身份、工具链、锁文件与空 Workspace；
                    RPC 和核心业务从后续里程碑开始。
                </p>
            </section>
        </main>
    );
}
