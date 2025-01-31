import { formatTimeFromMs } from './time';
type ConnectionState = 'disconnected' | 'connecting' | 'connected';
type Message = { CurrentTime: number } | { IsRunning: boolean };
type Command = { type: 'StartTimer' } | { type: 'StopTimer' } | { type: 'SetTime'; time: number };

export class TimerService {
	state: ConnectionState = $state('disconnected');
	time = $state(0);
	isRunning = $state(false);
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
			const msg = JSON.parse(event.data) as Message;
			if ('CurrentTime' in msg) {
				this.time = msg.CurrentTime;
			}
			if ('IsRunning' in msg) {
				this.isRunning = msg.IsRunning;
			}
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
