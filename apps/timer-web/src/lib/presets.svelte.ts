const presetTimes = [30 * 60, 15 * 60, 10 * 60, 5 * 60, 3 * 60, 1 * 60];

const migrations: [number, (db: IDBDatabase) => void][] = [
	[
		1,
		(db) => {
			console.log('migration to 1');
			const objectStore = db.createObjectStore('history', { keyPath: 'duration' });

			// Insert initial presets
			objectStore.transaction.oncomplete = () => {
				const store = db.transaction('history', 'readwrite').objectStore('history');
				presetTimes.forEach((time) => {
					store.add({ duration: time, count: 1 });
				});
			};
		}
	]
];

const CURRENT_VERSION = 1;
const DB_NAME = 'DurationHistory';
type HistoryItem = { duration: number; count: number };

function getDB(): Promise<IDBDatabase> {
	return new Promise((resolve, reject) => {
		const request = indexedDB.open(DB_NAME, CURRENT_VERSION);

		request.onsuccess = (event) => resolve((event.target as IDBOpenDBRequest).result);
		request.onerror = (event) => reject((event.target as IDBOpenDBRequest).error);
		request.onupgradeneeded = (event) => {
			const { result: db } = event.target as IDBOpenDBRequest;
			console.log('migrate from', event.oldVersion, 'to', event.newVersion);
			if (event.newVersion == null) {
				return;
			}

			for (let version = event.oldVersion + 1; event.newVersion >= version; version++) {
				const [, migrate] = migrations.find(([v]) => v === version) ?? [];
				if (!migrate) {
					throw new Error(`No migration for version ${version} was found`);
				}

				if (migrate) {
					migrate(db);
				}
			}
		};
	});
}

async function getAllHistoryItems(): Promise<HistoryItem[]> {
	const db = await getDB();
	const store = db.transaction('history').objectStore('history');

	return new Promise((resolve, reject) => {
		const items: { duration: number; count: number }[] = [];

		const request = store.openCursor();
		request.onerror = (ev) => {
			reject((ev.target as IDBRequest<IDBCursorWithValue | null>).error);
		};
		request.onsuccess = (ev) => {
			const cursor = (ev.target as IDBRequest<IDBCursorWithValue | null>).result;
			if (cursor) {
				items.push(cursor.value);
				cursor.continue();
			} else {
				resolve(items);
			}
		};
	});
}

function wrapRequest<T>(request: IDBRequest<T>): Promise<T> {
	return new Promise((resolve, reject) => {
		request.onsuccess = (ev) => resolve((ev.target as IDBRequest<T>).result);
		request.onerror = (ev) => reject((ev.target as IDBRequest<T>).error);
	});
}

function getItem<TData>(
	store: IDBObjectStore,
	keyPath: IDBValidKey | IDBKeyRange
): Promise<TData | undefined> {
	return wrapRequest(store.get(keyPath));
}

function putItem<T>(store: IDBObjectStore, item: T, key?: IDBValidKey) {
	return wrapRequest(store.put(item, key));
}

async function addDuration(duration: number) {
	const db = await getDB();
	const store = db.transaction('history', 'readwrite').objectStore('history');

	let item = await getItem<HistoryItem>(store, duration);
	if (item) {
		item.count += 1;
	} else {
		item = { duration, count: 1 };
	}

	await putItem(store, item);
}

export class PresetTimes {
	times: number[] = $state([]);

	async load() {
		this._updateHistory();
	}

	async _updateHistory() {
		const durations = await getAllHistoryItems();
		this.times = durations
			.toSorted(({ count: a }, { count: b }) => b - a)
			.map((d) => d.duration)
			.slice(0, 12)
			.toSorted((a, b) => b - a);
	}

	async add(time: number) {
		await addDuration(time);
		this._updateHistory();
	}
}
