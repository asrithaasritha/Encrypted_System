from fastapi import FastAPI
from pydantic import BaseModel
from typing import Optional
import spacy
import re
from datetime import datetime

app = FastAPI()
nlp = spacy.load("en_core_web_sm")

class TextInput(BaseModel):
    text: str


class ExtractedData(BaseModel):
    vendor: Optional[str] = None
    amount: Optional[str] = None
    date: Optional[str] = None
    due_date: Optional[str] = None
    warranty_period: Optional[str] = None
    category: str = "General"
    confidence: float = 0.0


# 🔥 AMOUNT DETECTION
def extract_amount(text):
    patterns = [
        r"(?i)(total|grand total|amount|amt)[^\d]{0,10}(\d{2,6}(\.\d{1,2})?)",
        r"₹\s?(\d{2,6}(\.\d{1,2})?)",
        r"Rs\.?\s?(\d{2,6}(\.\d{1,2})?)",
        r"\b(\d{2,6}\.\d{2})\b"
    ]

    values = []

    for pattern in patterns:
        matches = re.findall(pattern, text)
        for m in matches:
            try:
                val = float(m[1] if isinstance(m, tuple) else m)
                values.append(val)
            except:
                continue

    if values:
        return str(max(values))

    return None


@app.post("/extract", response_model=ExtractedData)
def extract(data: TextInput):
    text = data.text
    doc = nlp(text)

    result = ExtractedData()

    # Amount
    result.amount = extract_amount(text)

    # Date (NER)
    for ent in doc.ents:
        if ent.label_ == "DATE":
            result.date = ent.text
            break

    # ✅ FIXED: due_date logic (IMPORTANT)
    if result.date:
        result.due_date = result.date
    else:
        # fallback → today (so reminders always trigger)
        result.due_date = datetime.now().strftime("%Y-%m-%d")

    # Vendor
    for ent in doc.ents:
        if ent.label_ == "ORG":
            result.vendor = ent.text
            break

    if not result.vendor:
        for line in text.splitlines():
            line = line.strip()
            if len(line) > 3 and not line[0].isdigit():
                result.vendor = line
                break

    result.confidence = 0.85 if result.amount else 0.6

    return result


@app.get("/health")
def health():
    return {"status": "ok"}