import { Grid } from "./grid";
import Position from "./position";
import { Tile } from "./tile";

export interface ActuatorMetadata {
  score: number;
  over: boolean;
  won: boolean;
  bestScore: number;
  terminated: boolean;
  strength: number;
  aiIsOn(): boolean;
  throttleIsOn(): boolean;
}

export class HTMLActuator {
  private readonly tileContainer = document.querySelector(".tile-container")!;
  private readonly scoreContainer = document.querySelector(".score-container")!;
  private readonly bestContainer = document.querySelector(".best-container")!;
  private readonly strengthContainer = document.querySelector(
    ".strength-container"
  )!;
  private readonly runButton = document.querySelector(".run-button")!;
  private readonly throttleButton = document.querySelector(".throttle-button")!;
  private readonly messageContainer = document.querySelector(".game-message")!;
  private score = 0;
  public actuate(grid: Grid, metadata: ActuatorMetadata): Promise<void> {
    return new Promise(resolve => {
      window.requestAnimationFrame(() => {
        this.clearContainer(this.tileContainer);
        for (const column of grid.tiles) {
          for (const tile of column) {
            if (tile) {
              this.addTile(tile);
            }
          }
        }
        this.updateScore(metadata.score);
        this.updateBestScore(metadata.bestScore);
        this.updateStrength(metadata.strength);
        this.updateRunButton(metadata.aiIsOn());
        this.updateThrottleButton(metadata.throttleIsOn());
        if (metadata.terminated) {
          if (metadata.over) {
            this.message(false); // You lose
          } else if (metadata.won) {
            this.message(true); // You win!
          }
        }
        resolve();
      });
    });
  }
  // Continues the game (both restart and keep playing)
  public continueGame(): void {
    this.clearMessage();
  }
  public updateStrength(strength: number): void {
    this.strengthContainer.textContent = strength.toString();
  }
  public updateRunButton(aiIsOn: boolean): void {
    if (!aiIsOn) {
      this.runButton.textContent = "Start AI";
    } else {
      this.runButton.textContent = "Stop AI";
    }
  }
  public updateThrottleButton(throttleAi: boolean): any {
    if (throttleAi) {
      this.throttleButton.textContent = "Unthrottle";
    } else {
      this.throttleButton.textContent = "Throttle";
    }
  }
  private clearContainer(container: Element): void {
    while (container.firstChild) {
      container.removeChild(container.firstChild);
    }
  }
  private addTile(tile: Tile): void {
    const wrapper = document.createElement("div");
    const inner = document.createElement("div");
    const position = tile.previousPosition || { x: tile.x, y: tile.y };
    const positionClass = this.positionClass(position);
    // We can't use classlist because it somehow glitches when replacing classes
    const classes = ["tile", "tile-" + tile.value, positionClass];
    if (tile.value > 2048) classes.push("tile-super");
    this.applyClasses(wrapper, classes);
    inner.classList.add("tile-inner");
    inner.textContent = tile.value.toString();
    if (tile.previousPosition) {
      // Make sure that the tile gets rendered in the previous position first
      window.requestAnimationFrame(() => {
        classes[2] = this.positionClass({ x: tile.x, y: tile.y });
        this.applyClasses(wrapper, classes); // Update the position
      });
    } else if (tile.mergedFrom) {
      classes.push("tile-merged");
      this.applyClasses(wrapper, classes);
      // Render the tiles that merged
      for (const merged of tile.mergedFrom) {
        this.addTile(merged);
      }
    } else {
      classes.push("tile-new");
      this.applyClasses(wrapper, classes);
    }
    // Add the inner part of the tile to the wrapper
    wrapper.appendChild(inner);
    // Put the tile on the board
    this.tileContainer.appendChild(wrapper);
  }
  private applyClasses(element: Element, classes: string[]): void {
    element.setAttribute("class", classes.join(" "));
  }
  private normalizePosition(position: Position): Position {
    return { x: position.x + 1, y: position.y + 1 };
  }
  private positionClass(position: Position): string {
    position = this.normalizePosition(position);
    return "tile-position-" + position.x + "-" + position.y;
  }
  private updateScore(score: number): void {
    this.clearContainer(this.scoreContainer);
    const difference = score - this.score;
    this.score = score;
    this.scoreContainer.textContent = this.score.toString();
    if (difference > 0) {
      const addition = document.createElement("div");
      addition.classList.add("score-addition");
      addition.textContent = "+" + difference;
      this.scoreContainer.appendChild(addition);
    }
  }
  private updateBestScore(bestScore: number): void {
    this.bestContainer.textContent = bestScore.toString();
  }
  private message(won: boolean): void {
    const type = won ? "game-won" : "game-over";
    const message = won ? "You win!" : "Game over!";
    this.messageContainer.classList.add(type);
    this.messageContainer.getElementsByTagName("p")[0].textContent = message;
  }
  private clearMessage(): void {
    // IE only takes one value to remove at a time.
    this.messageContainer.classList.remove("game-won");
    this.messageContainer.classList.remove("game-over");
  }
}
