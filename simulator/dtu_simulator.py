#!/usr/bin/env python3
# -*- coding: utf-8 -*-

import json
import time
import random
import paho.mqtt.client as mqtt
from datetime import datetime, timedelta
import threading
import logging
from concurrent.futures import ThreadPoolExecutor, as_completed

logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

MQTT_BROKER = "localhost"
MQTT_PORT = 1883
MQTT_TOPIC = "oilfield/well/data"
MQTT_USERNAME = "admin"
MQTT_PASSWORD = "password"

INJECTION_WELL_COUNT = 300
PRODUCTION_WELL_COUNT = 500

WELL_BASE_DATA = {
    "injection": {},
    "production": {}
}

def generate_well_base_data():
    logger.info("Generating well base data...")
    
    center_lat, center_lon = 38.75, 116.25
    
    for i in range(INJECTION_WELL_COUNT):
        well_id = f"Z-{i+1:04d}"
        angle = random.uniform(0, 2 * 3.14159)
        radius = random.uniform(0.5, 8.0)
        lat = center_lat + radius * 0.008 * random.uniform(-1, 1)
        lon = center_lon + radius * 0.01 * random.uniform(-1, 1)
        
        block = f"区块-{random.choice(['A', 'B', 'C', 'D', 'E'])}"
        
        WELL_BASE_DATA["injection"][well_id] = {
            "wellId": well_id,
            "wellName": f"注水井-{i+1:04d}",
            "wellType": "INJECTION",
            "blockName": block,
            "latitude": lat,
            "longitude": lon,
            "designPressure": random.uniform(25.0, 35.0),
            "baseWaterVolume": random.uniform(80.0, 150.0),
            "basePressure": random.uniform(18.0, 28.0),
            "baseAbsorptionIndex": random.uniform(2.0, 6.0),
            "pressureTrend": random.uniform(-0.02, 0.05)
        }
    
    for i in range(PRODUCTION_WELL_COUNT):
        well_id = f"C-{i+1:04d}"
        angle = random.uniform(0, 2 * 3.14159)
        radius = random.uniform(0.5, 8.0)
        lat = center_lat + radius * 0.008 * random.uniform(-1, 1)
        lon = center_lon + radius * 0.01 * random.uniform(-1, 1)
        
        block = f"区块-{random.choice(['A', 'B', 'C', 'D', 'E'])}"
        
        WELL_BASE_DATA["production"][well_id] = {
            "wellId": well_id,
            "wellName": f"采油井-{i+1:04d}",
            "wellType": "PRODUCTION",
            "blockName": block,
            "latitude": lat,
            "longitude": lon,
            "baseFluidVolume": random.uniform(50.0, 120.0),
            "baseOilVolume": random.uniform(5.0, 25.0),
            "baseWaterCut": random.uniform(65.0, 92.0),
            "baseFluidLevel": random.uniform(800.0, 1500.0),
            "waterCutTrend": random.uniform(-0.01, 0.03),
            "oilDeclineRate": random.uniform(-0.02, -0.005)
        }
    
    logger.info(f"Generated {INJECTION_WELL_COUNT} injection wells and {PRODUCTION_WELL_COUNT} production wells")

def generate_injection_data(well_id, base_data, report_time):
    day_factor = 1.0 + 0.1 * random.uniform(-1, 1)
    pressure_variation = base_data["pressureTrend"] * (random.random() * 30)
    
    water_volume = base_data["baseWaterVolume"] * day_factor * (1 + random.uniform(-0.08, 0.08))
    pressure = base_data["basePressure"] + pressure_variation + random.uniform(-1.5, 1.5)
    absorption_index = base_data["baseAbsorptionIndex"] * (1 + random.uniform(-0.1, 0.1))
    
    if random.random() < 0.02:
        pressure = base_data["designPressure"] * random.uniform(0.85, 0.98)
    
    if random.random() < 0.005:
        pressure = base_data["designPressure"] * random.uniform(1.0, 1.1)
        logger.warning(f"Abnormal pressure for {well_id}: {pressure:.2f} MPa")
    
    return {
        "wellId": well_id,
        "wellType": "INJECTION",
        "reportTime": report_time.isoformat(),
        "waterVolume": round(water_volume, 2),
        "injectionPressure": round(max(0, pressure), 2),
        "absorptionIndex": round(max(0, absorption_index), 2)
    }

def generate_production_data(well_id, base_data, report_time):
    day_factor = 1.0 + 0.1 * random.uniform(-1, 1)
    water_cut_variation = base_data["waterCutTrend"] * (random.random() * 30)
    
    fluid_volume = base_data["baseFluidVolume"] * day_factor * (1 + random.uniform(-0.08, 0.08))
    oil_volume = base_data["baseOilVolume"] * (1 + base_data["oilDeclineRate"] * (random.random() * 30))
    oil_volume = oil_volume * (1 + random.uniform(-0.08, 0.08))
    
    water_cut = base_data["baseWaterCut"] + water_cut_variation + random.uniform(-1.5, 1.5)
    water_cut = max(0, min(99.5, water_cut))
    
    oil_volume = min(oil_volume, fluid_volume * (1 - water_cut / 100) * 0.95)
    
    fluid_level = base_data["baseFluidLevel"] + random.uniform(-50.0, 50.0)
    
    if random.random() < 0.02:
        water_cut = min(99.5, water_cut + random.uniform(3, 8))
        logger.warning(f"High water cut for {well_id}: {water_cut:.2f}%")
    
    if random.random() < 0.005:
        water_cut = min(99.5, water_cut + random.uniform(8, 15))
        logger.warning(f"Very high water cut for {well_id}: {water_cut:.2f}%")
    
    return {
        "wellId": well_id,
        "wellType": "PRODUCTION",
        "reportTime": report_time.isoformat(),
        "fluidVolume": round(max(0, fluid_volume), 2),
        "oilVolume": round(max(0, oil_volume), 2),
        "waterCut": round(water_cut, 2),
        "fluidLevel": round(max(0, fluid_level), 2)
    }

class DTUSimulator:
    def __init__(self):
        self.client = mqtt.Client(
            client_id=f"dtu-simulator-{int(time.time())}",
            clean_session=True
        )
        self.client.username_pw_set(MQTT_USERNAME, MQTT_PASSWORD)
        self.client.on_connect = self.on_connect
        self.client.on_publish = self.on_publish
        self.connected = False
        self.running = False
        self.publish_count = 0
        self.error_count = 0
    
    def on_connect(self, client, userdata, flags, rc):
        if rc == 0:
            self.connected = True
            logger.info("Connected to MQTT broker successfully")
        else:
            logger.error(f"Failed to connect to MQTT broker, return code: {rc}")
            self.connected = False
    
    def on_publish(self, client, userdata, mid):
        self.publish_count += 1
        if self.publish_count % 100 == 0:
            logger.info(f"Published {self.publish_count} messages")
    
    def connect(self):
        try:
            self.client.connect(MQTT_BROKER, MQTT_PORT, keepalive=60)
            self.client.loop_start()
            
            timeout = time.time() + 10
            while not self.connected and time.time() < timeout:
                time.sleep(0.1)
            
            return self.connected
        except Exception as e:
            logger.error(f"Connection error: {e}")
            return False
    
    def disconnect(self):
        self.running = False
        if self.client:
            self.client.loop_stop()
            self.client.disconnect()
        logger.info(f"Disconnected. Total published: {self.publish_count}, errors: {self.error_count}")
    
    def publish_data(self, data):
        if not self.connected:
            logger.warning("Not connected, trying to reconnect...")
            if not self.connect():
                self.error_count += 1
                return False
        
        try:
            payload = json.dumps(data, ensure_ascii=False)
            result = self.client.publish(
                MQTT_TOPIC,
                payload,
                qos=1,
                retain=False
            )
            
            if result.rc != mqtt.MQTT_ERR_SUCCESS:
                logger.warning(f"Publish failed: {result.rc}")
                self.error_count += 1
                return False
            
            return True
        except Exception as e:
            logger.error(f"Publish error: {e}")
            self.error_count += 1
            return False

def simulate_daily_report(simulator, start_date=None, end_date=None, speed_factor=1.0):
    if start_date is None:
        current_date = datetime.now().replace(hour=8, minute=0, second=0, microsecond=0)
    else:
        current_date = start_date
    
    if end_date is None:
        end_date = current_date
    
    total_days = (end_date - current_date).days + 1
    total_wells = INJECTION_WELL_COUNT + PRODUCTION_WELL_COUNT
    
    logger.info(f"Starting simulation from {current_date.date()} to {end_date.date()}")
    logger.info(f"Total wells: {total_wells}, Total days: {total_days}")
    
    day_count = 0
    while current_date <= end_date and simulator.running:
        logger.info(f"Reporting data for {current_date.date()}...")
        
        report_time = current_date
        
        with ThreadPoolExecutor(max_workers=20) as executor:
            futures = []
            
            for well_id, base_data in WELL_BASE_DATA["injection"].items():
                data = generate_injection_data(well_id, base_data, report_time)
                futures.append(executor.submit(simulator.publish_data, data))
            
            for well_id, base_data in WELL_BASE_DATA["production"].items():
                data = generate_production_data(well_id, base_data, report_time)
                futures.append(executor.submit(simulator.publish_data, data))
            
            for future in as_completed(futures):
                future.result()
        
        day_count += 1
        logger.info(f"Completed day {day_count}/{total_days}: {current_date.date()}")
        
        current_date += timedelta(days=1)
        
        if speed_factor > 0 and current_date <= end_date:
            sleep_time = 1.0 / speed_factor
            time.sleep(sleep_time)
    
    logger.info("Simulation completed")

def simulate_realtime(simulator, speed_factor=1.0):
    logger.info("Starting real-time simulation...")
    logger.info(f"Speed factor: {speed_factor}x")
    
    while simulator.running:
        current_time = datetime.now()
        
        if current_time.minute == 0:
            report_time = current_time.replace(second=0, microsecond=0)
            logger.info(f"Reporting data for {report_time}...")
            
            count = 0
            for well_id, base_data in WELL_BASE_DATA["injection"].items():
                data = generate_injection_data(well_id, base_data, report_time)
                simulator.publish_data(data)
                count += 1
                if count % 50 == 0:
                    time.sleep(0.01)
            
            for well_id, base_data in WELL_BASE_DATA["production"].items():
                data = generate_production_data(well_id, base_data, report_time)
                simulator.publish_data(data)
                count += 1
                if count % 50 == 0:
                    time.sleep(0.01)
            
            time.sleep(60)
        else:
            time.sleep(1)

def main():
    import argparse
    
    parser = argparse.ArgumentParser(description="4G DTU Data Simulator for Oilfield")
    parser.add_argument(
        "--mode",
        choices=["daily", "realtime", "backfill"],
        default="daily",
        help="Simulation mode: daily (single day), realtime (continuous), backfill (date range)"
    )
    parser.add_argument(
        "--start-date",
        type=str,
        default=None,
        help="Start date (YYYY-MM-DD) for backfill mode"
    )
    parser.add_argument(
        "--end-date",
        type=str,
        default=None,
        help="End date (YYYY-MM-DD) for backfill mode or daily mode"
    )
    parser.add_argument(
        "--speed",
        type=float,
        default=1.0,
        help="Simulation speed factor (default: 1.0)"
    )
    parser.add_argument(
        "--broker",
        type=str,
        default=MQTT_BROKER,
        help="MQTT broker address"
    )
    parser.add_argument(
        "--port",
        type=int,
        default=MQTT_PORT,
        help="MQTT broker port"
    )
    
    args = parser.parse_args()
    
    global MQTT_BROKER, MQTT_PORT
    MQTT_BROKER = args.broker
    MQTT_PORT = args.port
    
    generate_well_base_data()
    
    simulator = DTUSimulator()
    simulator.running = True
    
    try:
        if not simulator.connect():
            logger.error("Failed to connect to MQTT broker")
            return
        
        if args.mode == "backfill":
            if not args.start_date or not args.end_date:
                logger.error("Backfill mode requires --start-date and --end-date")
                return
            
            start_date = datetime.strptime(args.start_date, "%Y-%m-%d")
            end_date = datetime.strptime(args.end_date, "%Y-%m-%d")
            simulate_daily_report(simulator, start_date, end_date, args.speed)
        
        elif args.mode == "daily":
            if args.end_date:
                report_date = datetime.strptime(args.end_date, "%Y-%m-%d")
            else:
                report_date = datetime.now().replace(hour=8, minute=0, second=0, microsecond=0)
            
            simulate_daily_report(simulator, report_date, report_date, args.speed)
        
        elif args.mode == "realtime":
            simulate_realtime(simulator, args.speed)
    
    except KeyboardInterrupt:
        logger.info("Simulation stopped by user")
    except Exception as e:
        logger.error(f"Simulation error: {e}", exc_info=True)
    finally:
        simulator.disconnect()

if __name__ == "__main__":
    main()
