import { Component } from '@angular/core';

import { Converter } from './features/converter/converter';

@Component({
  selector: 'app-root',
  imports: [Converter],
  templateUrl: './app.html',
  styleUrl: './app.scss',
})
export class App {}
