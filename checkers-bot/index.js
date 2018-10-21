//const assert = require('assert');

const _ = require('lodash');
const async = require('async');

const jayson = require('jayson/promise');

const MAX_PLAY_TIMEOUT = 5 * 60 * 1000; // 5 minutes

// global variable
var my_player = null;

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
    return new Promise((resolve) => setTimeout(resolve, time));
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
    ret.players = [];
    ret.players.push(lines.shift().match(/player 1: (.*)/i)[1]);
    ret.players.push(lines.shift().match(/player 2: (.*)/i)[1]);
    
    ret.board = [];
    for(let line of lines) {
        if(line[2] != '|')
            continue;
        ret.board.push(line.substring(3, line.length - 1).replace(/\s/g, ''));
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
    return cols[x] + (y + 1).toString();
}

/// Investigate the possibility of moving the given checkers piece in the specified direction.
/// If a move is possible, then the moves array will be modified with the appropriate move
/// If a jump is possible, then the moves array will be prepended with the appropriate jump options
/// If no move is possible, the moves array is not modified.
/// jump_path and the arguments after used for recursion and should not be included
/// Returns the number of new moves added
function consider_move(moves, board, x, y, dx, dy, jump_path, hit_path, orig_x, orig_y) {
    
    if(board[y][x] == '.' && !jump_path)
        throw 'WARN: Consider_move called on empty board location: ' + xy_to_checkers_coord(x, y);
    
    if(board[y][x] == 'b' && dy < 0)
        return 0;
    
    if(board[y][x] == 'r' && dy > 0)
        return 0;
    
    if(x + dx < 0 || x + dx >= board[y].length || y + dy < 0 || y + dy >= board.length) {
        return 0;
    }
    
    let nx = x + 2 * dx;
    let ny = y + 2 * dy;
    
    let is_empty = board[y + dy][x + dx] == '.';
    let is_mine = board[y][x].toLowerCase() == board[y + dy][x + dx].toLowerCase();
    
    if(!jump_path && is_empty) {
        moves.push([xy_to_checkers_coord(x, y), 'move', delta_to_dir(dx, dy)]);
        return 1;
    }
    else if(!is_empty && !is_mine && 
        nx >= 0 && nx < board[y].length && ny >= 0 && ny < board.length &&
        board[ny][nx] == '.') {
        // try jumping
        // make sure we are not jumping over somewhere that has already been hit
        if(hit_path && hit_path.indexOf(xy_to_checkers_coord(x + dx, y + dy)) == -1)
            return 0;
        
        let added = 0;
        // 'slice' below makes a clone of the array references before we add our items
        jump_path = jump_path ? _.clone(jump_path) : [];
        hit_path = hit_path ? _.clone(hit_path) : [];
        jump_path.push(delta_to_dir(dx, dy));
        hit_path.push(xy_to_checkers_coord(x + dx, y + dy));
        
        if(orig_x == undefined) {
            orig_x = x;
            orig_y = y;
        }
        
        added += consider_move(moves, board, nx, ny, nx + 2, ny + 2, jump_path, hit_path, orig_x, orig_y);
        added += consider_move(moves, board, nx, ny, nx - 2, ny + 2, jump_path, hit_path, orig_x, orig_y);
        added += consider_move(moves, board, nx, ny, nx + 2, ny - 2, jump_path, hit_path, orig_x, orig_y);
        added += consider_move(moves, board, nx, ny, nx - 2, ny - 2, jump_path, hit_path, orig_x, orig_y);
        
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
function get_available_moves(board, is_red) {
    
    console.log('GET AVAILABLE MOVES AS RED:', is_red);
    console.log(board);
    
    let moves = [];
    
    for(let y = 0;y < board.length;y++) {
        for(let x = 0;x < board[y].length;x++) {
            
            if(
                ((board[y][x] == 'R' || board[y][x] == 'r') && is_red) ||
                ((board[y][x] == 'B' || board[y][x] == 'b') && !is_red)
            ) {
                consider_move(moves, board, x, y,  1, -1);
                consider_move(moves, board, x, y,  1,  1);
                consider_move(moves, board, x, y, -1, -1);
                consider_move(moves, board, x, y, -1,  1);   
            }
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
                let r = await client.request('new_checkers_game', args);
                
                if(!r.error) {
                    console.log('Created new game:', idx);
                    
                    // wait for status to be ready to play
                    let game = res;
                    while(game.status != 'active') {
                        console.log('Waiting for player to join game', idx);
                        await sleep(2000);
                        game = await get_checkers_board(idx);
                        
                        if(game.players[0] != my_player) {
                            console.log('Game was stolen by another... rebidding...');
                            return await bid(idx);
                        }
                    }
                    
                    // game should be ready to be played for now
                    return [idx, res.board, true];   
                }
                else {
                    console.log('Failed to start game: ', r.error);
                }
            }
            catch(err) {
                // ignore error for now (TODO: Could cause problems)
            }
        
            // have to check again
            res = await get_checkers_board(idx);
        }
        
        if(res.status == 'waiting for join') {
            try {
                let r = await client.request('join_checkers_game', idx_to_xy(idx));
                
                if(!r.error) {
                    console.log('Joined game:', idx);
                    
                    // wait for the first player to make the move (this also ensures we are the ones playing the game)
                    // wait for status to be ready to play
                    let orig_board = res.board;
                    let game = res;
                    while(_.isEqual(game.board, orig_board)) {
                        console.log('Waiting for opponents first move...');
                        await sleep(2000);
                        game = await get_checkers_board(idx);
                        
                        if(game.players[1] != my_player) {
                            console.log('Game was stolen by another... rebidding...');
                            idx++;
                            return await bid(idx);
                        }
                    }
                
                    return [idx, game.board, false];
                }
                else {
                    console.log('Failed to join game: ', r.error);
                }
            }
            catch(err) {
                // ignore error for now (TODO: Could cause problems)
            }
        }
        
        idx++;
    }
}

async function connect_and_register() {
    // make sure player is registered (for now ignore errors if they happen)
    let attempt = 0;
    while(attempt < 10) {
        try {
            let pid = await client.request('register_my_player', []);
            if(pid.result) {
                console.log('Registered as player', pid.result);
            }
            else {
                
                pid = await client.request('get_my_player', []);
                
                if(!pid.result) {
                    console.log('CRITICAL: Failed to load player information', pid.error);
                    return null;
                }
            }
            
            my_player = pid;
            
            console.log('Waiting to allow block sync for registration...');
            await sleep(30000);
            
            return pid.result;
            
        } catch(err) {
            console.error('WARN: Connection failed:', err);
            await sleep(1000);
        }
    }
}

async function main_loop() {

    console.log('Blockscape Checkers Bot Started');
    
    if(!(my_player = await connect_and_register())) {
        return;
    }

    let pos = 0;
    while(true) {
        // find a game to play
        pos = await autodial(pos);
        
        // try to join the game
        let r = await bid(pos);
        
        pos = r[0];
        let prev_board = r[1];
        let is_red = r[2];
        let available_moves = get_available_moves(prev_board, is_red);
        // play loop
        do {
            // select a random, valid move
            let xy = idx_to_xy(pos);
            
            console.log('Selecting from', available_moves.length, 'moves');
            let move = _.sample(available_moves);
            move.unshift(xy[1]);
            move.unshift(xy[0]);
            
            // should just be able to play it like this
            console.log('Play: ', move.join(' '));
            try {
                let r = await client.request('play_checkers', move);
                if(r.error)
                    throw r.error;
                
                prev_board = (await get_checkers_board(pos)).board;
            }
            catch(err) {
                console.error('Failed to play move:', err);
                // game over for right now
                break;
            }
            
            let start_wait = Date.now();
            let new_board = null;
            
            // wait for a move on the board
            do {
                await sleep(1000);
                new_board = (await get_checkers_board(pos)).board;
            } while(_.isEqual(new_board, prev_board) && Date.now() - start_wait < MAX_PLAY_TIMEOUT);
            
            if(Date.now() - start_wait >= MAX_PLAY_TIMEOUT) {
                console.error('Timed out waiting for other player move!');
                continue;
            }
            
            // refresh available moves
            available_moves = get_available_moves(new_board, is_red);
            prev_board = new_board;
        } while(available_moves.length);
        
        console.log('Game seems to be over! Moving on...');
    }
}

main_loop();
