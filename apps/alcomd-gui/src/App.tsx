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
                <p className="eyebrow">{productFamily} · v4 初始化仓库</p>
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
                        <strong>M-1 审计</strong>
                    </article>
                </div>
                <div className="actions">{materialButton}</div>
            </section>

            <section className="notice">
                <h2>先审计，再施工</h2>
                <p>
                    请先完成 <code>docs/exec-plans/M-1-audit.md</code>，冻结 v3 迁移输入与
                    VPM 生态兼容边界，然后才进入 RPC 和核心实现。
                </p>
            </section>
        </main>
    );
}
