//const assert = require('assert');

//const _ = require('lodash');
const async = require('async');

const jayson = require('jayson');

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
    return [idx / wrap, idx % wrap];
}

/// causes the function to delay in async/await code
function sleep(time) {
    return new Promise((resolve) => setTimeout(() => resolve(), time));
}

/// Runs the get_checkers_board rpc method, converting the board into
/// data readable by JS
async function get_checkers_board(idx) {
    let res = await client.request('get_checkers_board', idx_to_xy(idx));
    
    // parse
    let lines = res.result.split('\n');
    let ret = {};
    
    ret.status = lines.unshift().match(/status: (.*)/i)[1];
    
    ret.board = [];
    for(let line of lines) {
        if(line[2] != '|')
            continue;
        ret.board.push(line.substr(3, line.length - 1).replace(' ', ''));
    }
    
    return ret;
}

/// Using the given board, calculate all possible valid moves
/// An item in the array of the return value can be used directly as an RPC call
function get_available_moves(board) {
    return board;
}

/// finds either:
/// 1. A game which is waiting for a player to join
/// 2. The end of any started games
function autodial(start_at) {
    console.log('Begin autodial at', start_at);
    
    let idx = 0;
    let jump = 1 / 2;
    let up = false;
    let extend = false;

    return new Promise((resolve, reject) => {
        async.doWhilst(
            async function(callback) {
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
                    jump /= 2;
                    
                }
                
                idx += up ? jump : -jump;
                
                callback();
            },
            function() { return jump >= 1; },
            async function(err, n) {
                if(err) {
                    reject(err);
                }
				
                console.log('Completed autodial in', n, 'iterations');
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
        
        console.log('Try Join:', idx);
        
        res = res.result;
        
        
        if(res.status == 'not started') {
            let args = idx_to_xy(idx);
            args.push(0);
            try {
                await client.request('new_checkers_game', args);
                console.log('Created new game:', idx);
                return [idx, true, res.board];
            }
            catch(err) {
                // ignore error for now (TODO: Could cause problems)
            }
        }
        
        // have to check again
        res = await get_checkers_board(idx);
        
        if(res.status == 'waiting to join') {
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
    let pos = 0;

    // make sure player is registered (for now ignore errors if they happen)
    try {
        let pid = await client.request('register_my_player');
        console.log('Registered as player', pid.result);
    } catch(err) {
        console.log('WARN: Failed to register player (might already be registered):', err);
    }
    
    while(true) {
        // find a game to play
        pos = autodial(pos);
        
        // try to join the game
        let r = bid(pos);
        
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
