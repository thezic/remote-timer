import { formatTimeFromMs } from './time';
type ConnectionState = 'disconnected' | 'connecting' | 'connected';
type Message = { current_time: number; is_running: boolean; client_count: number };
type Command = { type: 'StartTimer' } | { type: 'StopTimer' } | { type: 'SetTime'; time: number };

export class TimerService {
	state: ConnectionState = $state('disconnected');
	time = $state(0);
	isRunning = $state(false);
	numberOfClients = $state(0);

	formattedTime = $derived(formatTimeFromMs(this.time));

	_socket: WebSocket | null = null;

	connect(url: string) {
		if (WebSocket == undefined) {
			return;
		}
		this.state = 'connecting';
		const socket = new WebSocket(url);

		socket.addEventListener('open', () => {
			this.state = 'connected';
			this._socket = socket;
		});

		const reconnect = () => {
			this.state = 'disconnected';
			this._socket = null;
			setTimeout(() => this.connect(url), 1000);
		};

		socket.addEventListener('close', reconnect);
		socket.addEventListener('error', reconnect);

		socket.addEventListener('message', (event) => {
			console.log('Received', event.data);
			const msg = JSON.parse(event.data) as Message;
			this.time = msg.current_time;
			this.isRunning = msg.is_running;
			this.numberOfClients = msg.client_count;
		});
	}

	_sendMessage(command: Command) {
		if (this._socket) {
			console.log('Sending', command);
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
