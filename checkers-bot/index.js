//const assert = require('assert');

//const _ = require('lodash');
const async = require('async');

const jayson = require('jayson/promise');

const MAX_PLAY_TIMEOUT = 5 * 60 * 1000; // 5 minutes

// jsonrpc client for blockscape
var client = jayson.client.http({
    host: process.env.BLOCKSCAPE_HOST || 'localhost',
    port: process.env.BLOCKSCAPE_PORT || 8356
});

/// this bot uses a special mapping of an absolute index, rather than an x/y value.
/// this has to be converted when comming the API, however.
function idx_to_xy(idx) {
    let wrap = Math.pow(2, 31);
    return [Math.floor(idx / wrap).toString(), (idx % wrap).toString()];
}

/// causes the function to delay in async/await code
function sleep(time) {
    return new Promise((resolve) => setTimeout(() => resolve(), time));
}

/// Runs the get_checkers_board rpc method, converting the board into
/// data readable by JS
async function get_checkers_board(idx) {
    let res = await client.request('get_checkers_board', idx_to_xy(idx));

    if(!res.result) {
        throw res.error.message;
    }
    
    // parse
    let lines = res.result.split('\n');
    let ret = {};
    
    ret.status = lines.shift().match(/status: (.*)/i)[1];
    
    ret.board = [];
    for(let line of lines) {
        if(line[2] != '|')
            continue;
        ret.board.push(line.substr(3, line.length - 1).replace(/ /g, ''));
    }
    
    return ret;
}

function delta_to_dir(dx, dy) {
    if(dx > 0 && dy > 0) {
        return 'se';
    }
    else if(dx < 0 && dy > 0) {
        return 'sw';
    }
    else if(dx < 0 && dy < 0) {
        return 'nw';
    }
    else return 'ne';
}

function xy_to_checkers_coord(x, y) {
    let cols = 'abcdefgh';
    return cols[x] + y.toString();
}

/// Investigate the possibility of moving the given checkers piece in the specified direction.
/// If a move is possible, then the moves array will be modified with the appropriate move
/// If a jump is possible, then the moves array will be prepended with the appropriate jump options
/// If no move is possible, the moves array is not modified.
/// jump_path and the arguments after used for recursion and should not be included
/// Returns the number of new moves added
function consider_move(moves, board, x, y, dx, dy, jump_path, hit_path, orig_x, orig_y) {
    
    if(board[y][x] == 'b' && y < 0)
        return 0;
    
    if(board[y][x] == 'r' && y > 0)
        return 0;
    
    if(x + dx < 0 || x + dx >= board[y].length || y + dy < 0 || y + dy >= board.length) {
        return 0;
    }
    
    let nx = x + 2 * dx;
    let ny = y + 2 * dy;
    
    let is_empty = board[y + dy][x + dx] == '.';
    
    if(!jump_path && is_empty) {
        moves.push([xy_to_checkers_coord(x, y), 'move', delta_to_dir(dx, dy)]);
        return 1;
    }
    else if(!is_empty && nx >= 0 && nx < board[y].length && ny >= 0 && ny < board.length) {
        // try jumping
        // make sure we are not jumping over somewhere that has already been hit
        if(hit_path && hit_path.indexOf(xy_to_checkers_coord(x + dx, y + dy)))
            return 0;
        
        let added = 0;
        // 'slice' below makes a clone of the array references before we add our items
        jump_path = jump_path.slice(0) || [];
        hit_path = hit_path.slice(0) || [];
        jump_path.push(delta_to_dir(dx, dy));
        hit_path.push(xy_to_checkers_coord(x + dx, y + dy));
        
        if(orig_x == undefined) {
            orig_x = x;
            orig_y = y;
        }
        
        added += consider_move(moves, board, nx, ny, nx + 2, ny + 2, jump_path, hit_path, orig_x, orig_y);
        added += consider_move(moves, board, nx, ny, nx - 2, ny + 2, jump_path, hit_path, orig_x, orig_y);
        added += consider_move(moves, board, nx, ny, nx + 2, ny - 2, jump_path, orig_x, orig_y);
        added += consider_move(moves, board, nx, ny, nx - 2, ny - 2, jump_path, orig_x, orig_y);
        
        if(!added) {
            moves.push([xy_to_checkers_coord(orig_x, orig_y), 'jump'].concat(jump_path));
            return 1;
        }
        
        return added;
    }
    
    return 0;
}

/// Using the given board, calculate all possible valid moves
/// An item in the array of the return value can be used directly as an RPC call
function get_available_moves(board) {
    
    let moves = [];
    
    for(let y = 0;y < board.length;y++) {
        for(let x = 0;x < board[y].length;x++) {
            consider_move(moves, board, y, x,  1, -1);
            consider_move(moves, board, y, x,  1,  1);
            consider_move(moves, board, y, x, -1, -1);
            consider_move(moves, board, y, x, -1,  1);
        }
    }
    
    return moves;
}

/// finds either:
/// 1. A game which is waiting for a player to join
/// 2. The end of any started games
function autodial(start_at) {
    console.log('Begin autodial at', start_at);

    return new Promise((resolve, reject) => {
        let idx = start_at;
        let jump = 1 / 2;
        let up = true;
        let extend = true;
            
        async.doWhilst(
            async function() {
                let game = await get_checkers_board(idx);
                
                if(extend) {
                    if(game.status != 'active') {
                        idx += 1; // here we bump by one because this might be the first game to be included
                        extend = false; // this will take us directly into the !extend conditional below
                    }
                    else {
                        jump *= 2;
                    }
                }
                
                if(!extend) {
                    up = game.status == 'active';
                    jump = Math.floor(jump / 2);
                }
                
                idx += up ? jump : -jump;
                
                //callback();
                return null;
            },
            function() { return jump >= 1 / 2; },
            async function(err) {
                if(err) {
                    console.log(err);
                    return reject(err);
                }
				
                console.log('Completed autodial (new index is', idx, ')');
                //assert((await get_checkers_board(idx)).status != 'active', 'Should be an uninitialized checkers board');
                resolve(idx);
            }
        );
    });
}

/// Try to join (or start) a game at the given index. If the given index is not possible, increment and try again.
/// Keep trying until the game is playable.
async function bid(idx) {
    while(true) {
        let res = await get_checkers_board(idx);
        
        console.log('Bid:', idx);
        if(res.status == 'not started') {
            let args = idx_to_xy(idx);
            args.push('0');
            try {
                await client.request('new_checkers_game', args);
                console.log('Created new game:', idx);
                return [idx, true, res.board];
            }
            catch(err) {
                // ignore error for now (TODO: Could cause problems)
            }
        
            // have to check again
            res = await get_checkers_board(idx);
        }
        
        if(res.status == 'waiting for join') {
            try {
                await client.request('join_checkers_game', idx_to_xy(idx));
                console.log('Joined game:', idx);
                return [idx, false, res.board];
            }
            catch(err) {
                // ignore error for now (TODO: Could cause problems)
            }
        }
        
        idx++;
    }
}

async function main_loop() {

    console.log('Blockscape Checkers Bot Started');

    let pos = 0;

    // make sure player is registered (for now ignore errors if they happen)
    try {
        let pid = await client.request('register_my_player', []);
        if(pid.result) {
            console.log('Registered as player', pid.result);
        }
        else {
            console.log('WARN: Failed to register player (might already be registered):', pid.error);
        }
    } catch(err) {
        console.error('Connection failed:', err);
    }
    
    while(true) {
        // find a game to play
        pos = await autodial(pos);
        
        // try to join the game
        let r = await bid(pos);
        
        pos = r[0];
        let play_now = r[1];    
        let prev_board = r[2];
        let available_moves = [];
        // play loop
        do {
            if(play_now) {
                
                // select a random, valid move
            }
            else
                play_now = true;
            
            let start_wait = Date.now();
            let new_board = null;
            
            // wait for a move on the board
            do {
                await sleep(1000);
                new_board = await get_checkers_board(pos);
            } while(new_board == prev_board && Date.now() - start_wait > MAX_PLAY_TIMEOUT);
            
            // refresh available moves
            available_moves = get_available_moves(new_board);
            prev_board = new_board;
        } while(available_moves.length);
    }
}

main_loop();
