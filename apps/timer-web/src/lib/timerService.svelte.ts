import { formatTimeFromSeconds, seconds } from './time';
import { retry, tryit } from 'radash';

export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'closed';
type Message = {
	current_time: number;
	is_running: boolean;
	client_count: number;
	target_time: number;
};
type Command = { type: 'StartTimer' } | { type: 'StopTimer' } | { type: 'SetTime'; time: number };

class AbortError extends Error {
	constructor() {
		super('abort');
		Object.setPrototypeOf(this, new.target.prototype);
	}
}
function connect(url: string): Promise<WebSocket> {
	return new Promise((resolve, reject) => {
		const socket = new WebSocket(url);

		socket.addEventListener('open', () => resolve(socket), { once: true });
		socket.addEventListener('error', reject, { once: true });
	});
}

async function connectWithRetry(url: string, signal: AbortSignal): Promise<WebSocket | undefined> {
	return await retry({ delay: 1000, times: Infinity }, (exit) => {
		if (signal.aborted) {
			exit(new AbortError());
		}
		return connect(url);
	});
}

export class TimerService {
	state: ConnectionState = $state('disconnected');
	time = $state(0);
	targetTime = $state(0);
	isRunning = $state(false);
	numberOfClients = $state(0);

	timeSeconds = $derived(seconds(this.time));
	targetSeconds = $derived(seconds(this.targetTime));
	remainingSeconds = $derived(this.targetSeconds - this.timeSeconds);

	_socket: WebSocket | undefined;
	_abortController: AbortController | undefined;
	_url: string | undefined;

	async connect(url: string) {
		if (WebSocket == undefined) {
			return;
		}
		this.state = 'connecting';
		this.numberOfClients = 0;

		if (this._abortController) {
			this._abortController.abort();
		}
		this._abortController = new AbortController();
		const signal = this._abortController.signal;

		const [error, socket] = await tryit(connectWithRetry)(url, signal);
		this._socket = socket;

		if (!socket) {
			this.state = 'disconnected';
			if (error instanceof AbortError) {
				return;
			} else {
				throw error;
			}
		}

		this._wireSocket(socket, url);
		this.state = 'connected';
	}

	close() {
		this.state = 'closed';
		this._socket?.close();
		this._socket = undefined;

		if (this._abortController) {
			this._abortController.abort();
			this._abortController = undefined;
		}
	}

	_wireSocket(socket: WebSocket, url: string) {
		const reconnect = () => {
			this._socket = undefined;
			if (this.state == 'closed') {
				return;
			}
			this.connect(url);
		};

		socket.addEventListener('close', reconnect, { once: true });
		socket.addEventListener('error', reconnect, { once: true });

		socket.addEventListener('message', (event) => {
			const msg = JSON.parse(event.data) as Message;
			this.time = msg.current_time;
			this.targetTime = msg.target_time;
			this.isRunning = msg.is_running;
			this.numberOfClients = msg.client_count;
		});
	}

	_sendMessage(command: Command) {
		if (this._socket) {
			this._socket.send(JSON.stringify(command));
		}
	}

	startTimer() {
		this._sendMessage({ type: 'StartTimer' });
	}

	stopTimer() {
		this._sendMessage({ type: 'StopTimer' });
	}

	setTime(seconds: number) {
		this._sendMessage({ type: 'SetTime', time: seconds * 1000 });
	}
}
