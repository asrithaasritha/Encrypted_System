const state = {
	expenses: [],
	reminders: [],
};

const hasTauriInvoke =
	typeof window !== "undefined" &&
	window.__TAURI__ &&
	window.__TAURI__.invoke;

const tauriInvoke = async (cmd, args = {}) => {
	if (hasTauriInvoke) {
		return window.__TAURI__.invoke(cmd, args);
	}

	// Browser fallback for visual development.
	if (cmd === "unlock_vault") {
		if (args.password && args.password.length >= 3) {
			return { ok: true, data: "unlocked", error: null };
		}
		return { ok: false, data: null, error: "Invalid password" };
	}

	if (cmd === "list_expenses") {
		return {
			ok: true,
			data: [
				{
					id: 1,
					vendor: "Cafe August",
					amount: "320.00",
					date: "2026-04-07",
					category: "Food",
					due_date: "2026-04-12",
					confidence: 0.91,
				},
				{
					id: 2,
					vendor: "Nova Books",
					amount: "1080.00",
					date: "2026-04-01",
					category: "Shopping",
					due_date: "2026-04-09",
					confidence: 0.87,
				},
			],
			error: null,
		};
	}

	if (cmd === "list_reminders") {
		return {
			ok: true,
			data: [
				{
					id: 12,
					message: "Pay internet bill",
					remind_on: "2026-04-10",
				},
			],
			error: null,
		};
	}

	if (cmd === "mark_reminder_done") {
		return { ok: true, data: "done", error: null };
	}

	if (cmd === "check_reminders_now") {
		return { ok: true, data: "1 notifications fired", error: null };
	}

	if (cmd === "export_vault") {
		return { ok: true, data: "vault_export.db", error: null };
	}

	if (cmd === "scan_image") {
		return {
			ok: true,
			data: {
				id: Date.now(),
				vendor: "Scanned Merchant",
				amount: "650.00",
				date: new Date().toISOString().slice(0, 10),
				category: "General",
				due_date: new Date().toISOString().slice(0, 10),
				confidence: 0.84,
			},
			error: null,
		};
	}

	return { ok: false, data: null, error: "Unsupported command in browser mode" };
};

const fmtMoney = (n) => {
	const num = Number.parseFloat(String(n || "0").replace(/,/g, ""));
	if (Number.isNaN(num)) {
		return "INR 0";
	}
	return new Intl.NumberFormat("en-IN", {
		style: "currency",
		currency: "INR",
		maximumFractionDigits: 2,
	}).format(num);
};

const escapeHtml = (s) =>
	String(s ?? "")
		.replace(/&/g, "&amp;")
		.replace(/</g, "&lt;")
		.replace(/>/g, "&gt;")
		.replace(/\"/g, "&quot;")
		.replace(/'/g, "&#039;");

const setScreen = (screenId) => {
	document.querySelectorAll(".screen").forEach((el) => el.classList.remove("active"));
	const target = document.getElementById(screenId);
	if (target) {
		target.classList.add("active");
	}
};

const toast = (msg, kind = "ok") => {
	let host = document.getElementById("toast-host");
	if (!host) {
		host = document.createElement("div");
		host.id = "toast-host";
		document.body.appendChild(host);
	}

	const t = document.createElement("div");
	t.className = `toast ${kind}`;
	t.textContent = msg;
	host.appendChild(t);

	window.setTimeout(() => t.classList.add("hide"), 2200);
	window.setTimeout(() => t.remove(), 2700);
};

const renderMetrics = () => {
	const metricsEl = document.getElementById("metrics");
	if (!metricsEl) {
		return;
	}

	const count = state.expenses.length;
	const total = state.expenses.reduce((acc, e) => {
		const n = Number.parseFloat(String(e.amount || "0").replace(/,/g, ""));
		return acc + (Number.isNaN(n) ? 0 : n);
	}, 0);
	const avg = count > 0 ? total / count : 0;
	const dueSoon = state.expenses.filter((e) => {
		if (!e.due_date) {
			return false;
		}
		const due = new Date(e.due_date);
		const now = new Date();
		const days = (due - now) / (1000 * 60 * 60 * 24);
		return days >= -1 && days <= 7;
	}).length;

	metricsEl.innerHTML = `
		<article class="metric-card reveal"><p>Total Expenses</p><strong>${count}</strong></article>
		<article class="metric-card reveal"><p>Spent</p><strong>${fmtMoney(total)}</strong></article>
		<article class="metric-card reveal"><p>Avg Expense</p><strong>${fmtMoney(avg)}</strong></article>
		<article class="metric-card reveal"><p>Due in 7 days</p><strong>${dueSoon}</strong></article>
	`;
};

const renderExpenses = () => {
	const list = document.getElementById("exp-list");
	if (!list) {
		return;
	}

	if (!state.expenses.length) {
		list.innerHTML = `<div class="empty">No expenses yet. Scan your first bill to get started.</div>`;
		return;
	}

	const rows = [...state.expenses]
		.sort((a, b) => String(b.date).localeCompare(String(a.date)))
		.slice(0, 8)
		.map(
			(e) => `
			<article class="expense-card reveal">
				<div>
					<h4>${escapeHtml(e.vendor || "Unknown vendor")}</h4>
					<p>${escapeHtml(e.category || "General")} · ${escapeHtml(e.date || "-")}</p>
				</div>
				<div class="expense-right">
					<strong>${fmtMoney(e.amount)}</strong>
					<small>${e.confidence ? `conf ${Math.round(e.confidence * 100)}%` : ""}</small>
				</div>
			</article>`
		)
		.join("");

	list.innerHTML = rows;
};

const renderReminders = () => {
	const list = document.getElementById("rem-list");
	if (!list) {
		return;
	}

	if (!state.reminders.length) {
		list.innerHTML = `<div class="empty">No pending reminders.</div>`;
		return;
	}

	list.innerHTML = state.reminders
		.map(
			(r) => `
			<article class="rem-card reveal">
				<div>
					<h4>${escapeHtml(r.message)}</h4>
					<p>Remind on ${escapeHtml(r.remind_on)}</p>
				</div>
				<button class="ghost-btn" onclick="markDone(${Number(r.id)})">Mark done</button>
			</article>`
		)
		.join("");
};

const applyRevealAnimations = () => {
	const items = document.querySelectorAll(".reveal");
	if (!("IntersectionObserver" in window)) {
		items.forEach((el) => el.classList.add("show"));
		return;
	}

	const observer = new IntersectionObserver(
		(entries, obs) => {
			entries.forEach((entry) => {
				if (entry.isIntersecting) {
					entry.target.classList.add("show");
					obs.unobserve(entry.target);
				}
			});
		},
		{ threshold: 0.12 }
	);

	items.forEach((el) => observer.observe(el));
};

async function doUnlock() {
	const pw = document.getElementById("pw");
	const err = document.getElementById("unlock-err");
	if (!pw || !err) {
		return;
	}

	err.textContent = "";
	const password = pw.value.trim();
	if (!password) {
		err.textContent = "Please enter vault password.";
		return;
	}

	const btn = document.querySelector("#screen-unlock button");
	if (btn) {
		btn.disabled = true;
	}

	try {
		const res = await tauriInvoke("unlock_vault", { password });
		if (!res.ok) {
			err.textContent = res.error || "Could not unlock vault.";
			return;
		}

		setScreen("screen-dash");
		showTab("dash");
		await Promise.all([loadDash(), loadReminders()]);
		toast("Vault unlocked", "ok");
	} catch (e) {
		err.textContent = e && e.message ? e.message : "Unlock failed.";
	} finally {
		if (btn) {
			btn.disabled = false;
		}
	}
}

function showTab(name) {
	document.querySelectorAll(".tab").forEach((el) => el.classList.remove("active"));
	document.querySelectorAll(".ni").forEach((el) => el.classList.remove("active"));

	const activeTab = document.getElementById(`tab-${name}`);
	if (activeTab) {
		activeTab.classList.add("active");
	}

	const nav = document.querySelector(`.ni[onclick*="showTab('${name}')"]`);
	if (nav) {
		nav.classList.add("active");
	}

	if (name === "reminders") {
		loadReminders();
	}
}

async function loadDash() {
	const list = document.getElementById("exp-list");
	if (list) {
		list.innerHTML = `<div class="loading">Loading expenses...</div>`;
	}

	const res = await tauriInvoke("list_expenses");
	if (!res.ok) {
		if (list) {
			list.innerHTML = `<div class="empty">${escapeHtml(res.error || "Could not load expenses")}</div>`;
		}
		return;
	}

	state.expenses = Array.isArray(res.data) ? res.data : [];
	renderMetrics();
	renderExpenses();
	applyRevealAnimations();
}

async function loadReminders() {
	const list = document.getElementById("rem-list");
	if (list) {
		list.innerHTML = `<div class="loading">Loading reminders...</div>`;
	}

	const res = await tauriInvoke("list_reminders");
	if (!res.ok) {
		if (list) {
			list.innerHTML = `<div class="empty">${escapeHtml(res.error || "Could not load reminders")}</div>`;
		}
		return;
	}

	state.reminders = Array.isArray(res.data) ? res.data : [];
	renderReminders();
	applyRevealAnimations();
}

async function markDone(id) {
	const res = await tauriInvoke("mark_reminder_done", { id });
	if (!res.ok) {
		toast(res.error || "Could not update reminder", "err");
		return;
	}
	toast("Reminder marked done", "ok");
	await loadReminders();
}

async function checkNow() {
	const res = await tauriInvoke("check_reminders_now");
	if (!res.ok) {
		toast(res.error || "Reminder check failed", "err");
		return;
	}

	toast(res.data || "Reminder check complete", "ok");
	await loadReminders();
}

async function doExport() {
	const msg = document.getElementById("exp-msg");
	if (msg) {
		msg.textContent = "Exporting...";
	}

	const res = await tauriInvoke("export_vault");
	if (!res.ok) {
		if (msg) {
			msg.textContent = res.error || "Export failed";
		}
		toast(res.error || "Export failed", "err");
		return;
	}

	if (msg) {
		msg.textContent = `Exported: ${res.data}`;
	}
	toast("Vault exported", "ok");
}

async function scanFromPath(path) {
	const out = document.getElementById("scan-out");
	if (!out) {
		return;
	}

	out.innerHTML = `<div class="loading">Scanning...</div>`;
	const res = await tauriInvoke("scan_image", { path });
	if (!res.ok) {
		out.innerHTML = `<div class="empty">${escapeHtml(res.error || "Scan failed")}</div>`;
		return;
	}

	const e = res.data;
	out.innerHTML = `
		<article class="scan-result reveal show">
			<h4>Scanned Successfully</h4>
			<p><strong>Vendor:</strong> ${escapeHtml(e.vendor || "Unknown")}</p>
			<p><strong>Amount:</strong> ${fmtMoney(e.amount)}</p>
			<p><strong>Date:</strong> ${escapeHtml(e.date || "-")}</p>
			<p><strong>Category:</strong> ${escapeHtml(e.category || "General")}</p>
		</article>
	`;

	await loadDash();
}

async function handleFile(input) {
	const file = input && input.files && input.files[0];
	if (!file) {
		return;
	}

	const out = document.getElementById("scan-out");
	const candidatePath = file.path || "";

	if (!candidatePath) {
		if (out) {
			out.innerHTML =
				`<div class="empty">Could not get local file path. Use Tauri desktop build for scanner integration.</div>`;
		}
		return;
	}

	await scanFromPath(candidatePath);
}

async function handleDrop(event) {
	event.preventDefault();
	const file = event.dataTransfer && event.dataTransfer.files && event.dataTransfer.files[0];
	if (!file) {
		return;
	}

	const fakeInput = { files: [file] };
	await handleFile(fakeInput);
}

function lockVault() {
	state.expenses = [];
	state.reminders = [];
	const pw = document.getElementById("pw");
	const err = document.getElementById("unlock-err");
	if (pw) {
		pw.value = "";
	}
	if (err) {
		err.textContent = "";
	}
	setScreen("screen-unlock");
	toast("Vault locked", "ok");
}

document.addEventListener("DOMContentLoaded", () => {
	document.body.classList.add("ready");
	applyRevealAnimations();

	const drop = document.getElementById("drop");
	if (drop) {
		drop.addEventListener("dragenter", () => drop.classList.add("dragging"));
		drop.addEventListener("dragleave", () => drop.classList.remove("dragging"));
		drop.addEventListener("drop", () => drop.classList.remove("dragging"));
	}
});

window.doUnlock = doUnlock;
window.showTab = showTab;
window.loadDash = loadDash;
window.lockVault = lockVault;
window.handleDrop = handleDrop;
window.handleFile = handleFile;
window.checkNow = checkNow;
window.doExport = doExport;
window.markDone = markDone;
