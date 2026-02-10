import pandas as pd
import matplotlib.pyplot as plt
import sys

# Dateiname hier anpassen oder als Argument übergeben
filename = "vasili_log.csv"
if len(sys.argv) > 1:
    filename = sys.argv[1]

try:
    # 1. Daten einlesen (Vassili schreibt keine Header)
    # Format: Timestamp, Type, IP, Latency, Status
    cols = ["Timestamp", "Type", "Target", "Latency", "Status"]
    df = pd.read_csv(filename, names=cols, header=None)
    
    # Zeitstempel parsen
    df["Timestamp"] = pd.to_datetime(df["Timestamp"])
    
except FileNotFoundError:
    print(f"Fehler: Datei '{filename}' nicht gefunden.")
    sys.exit(1)

# 2. Daten trennen (Internet vs. Gateway)
internet = df[df["Type"] == "Internet"].copy()
gateway = df[df["Type"] == "Gateway"].copy() # Oder "Router", je nach Version

# 3. Jitter berechnen (Differenz zum vorherigen Ping)
# Wir berechnen das nur für erfolgreiche Pings (Status == OK)
internet_ok = internet[internet["Status"] == "OK"].copy()
internet_ok["Jitter"] = internet_ok["Latency"].diff().abs().fillna(0)

gateway_ok = gateway[gateway["Status"] == "OK"].copy()
gateway_ok["Jitter"] = gateway_ok["Latency"].diff().abs().fillna(0)

# 4. Packet Loss filtern
net_loss = internet[internet["Status"] == "TIMEOUT"]
gw_loss = gateway[gateway["Status"] == "TIMEOUT"]

# --- PLOTTING ---
plt.figure(figsize=(16, 8))
plt.style.use('dark_background') # Passt zum Terminal-Look

# Internet (Grün & Gelb)
plt.plot(internet_ok["Timestamp"], internet_ok["Latency"], 
         label=f"Internet Ping ({internet_ok['Target'].iloc[0] if not internet_ok.empty else '?'})", 
         color="#00ff00", linewidth=1, alpha=0.9)
plt.plot(internet_ok["Timestamp"], internet_ok["Jitter"], 
         label="Internet Jitter", color="#ffff00", linewidth=0.5, alpha=0.6)

# Gateway (Blau & Magenta) - falls vorhanden
if not gateway_ok.empty:
    plt.plot(gateway_ok["Timestamp"], gateway_ok["Latency"], 
             label=f"Gateway Ping ({gateway_ok['Target'].iloc[0]})", 
             color="#00ccff", linewidth=1, alpha=0.9)
    plt.plot(gateway_ok["Timestamp"], gateway_ok["Jitter"], 
             label="Gateway Jitter", color="#ff00ff", linewidth=0.5, alpha=0.6)

# Packet Loss (Kreuze)
# Wir setzen die Marker auf Y=100 (oder leicht darüber), damit man sie sieht
y_loss_level = internet_ok["Latency"].max() if not internet_ok.empty else 100
if y_loss_level < 50: y_loss_level = 50

if not net_loss.empty:
    plt.scatter(net_loss["Timestamp"], [y_loss_level] * len(net_loss), 
                color="red", marker="x", s=100, label="Internet Loss", zorder=10)

if not gw_loss.empty:
    plt.scatter(gw_loss["Timestamp"], [y_loss_level] * len(gw_loss), 
                color="magenta", marker="x", s=100, label="Gateway Loss", zorder=10)

# Labels & Titel
plt.title(f"VASSILI Network Analysis - {filename}", fontsize=14, color="white")
plt.ylabel("Latency / Jitter (ms)")
plt.xlabel("Time")
plt.grid(True, which='both', linestyle='--', linewidth=0.5, alpha=0.3)
plt.legend(loc="upper left")

plt.tight_layout()

# Generiere Dateinamen für das Bild (ersetzt .csv durch .png)
output_filename = filename.replace(".csv", ".png")
if output_filename == filename:
    output_filename += ".png"

plt.savefig(output_filename, dpi=100)
print(f"Plot gespeichert als: {output_filename}")