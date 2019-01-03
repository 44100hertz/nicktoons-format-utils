// Convert proprietary nicktoons-game .ini file into .json, which can later
// be actually used.
//
// TODO: json isn't the best at this! I know how to make json nice
// for the later code to parse, but I don't know if it's the best way overall.
// There may be some benefit to porting this to rust, and skipping json
// entirely. Either way, this is a good description of every quirk of the format.
//
// My format represents everything as {type: type, value: value}, with the value
// being the closest json representation to the ingame value, and the type
// Integer, Floating, String, Ident, Symbol, Bool, List, ExtraInfo, or Entity.
// Currently, arrays and objects are not typed in this way.
'use strict';

const fs = require('fs');

// Every posssible valid token. I'm only a bit lazy here.
const token_def = {
    // Integers and floats are separate.
    Integer: /(-?[0-9]+)/y,
    // Floats don't end in f sometimes. Press f for this format.
    // Only one file has an 'e' in a float, and it's e-006 when it should be 0.
    Floating: /([-0-9e\.]+)f?/y,
    // Strings have no escapes.
    String: /"(.*?)"/y,
    // Identifiers are Capitalized and may have _
    Ident: /([A-Z][\w]*)/y,
    Symbol: /([={}])/y, // The only 3 symbols with meaning.
    Bool: /(true|false)/y,
    // I treat ; and , as whitespace because they do nothing.
    // "Inconvenience characters"
    Whitespace: /([\s\r\n;,]*|\/\/[^\n]*\n)*/y,
};

// Find the max-length valid token in a string
function get_token (input, index) {
    let len = 0, type, value;
    for (let k in token_def) {
	const regex = token_def[k];
	regex.lastIndex = index; // Match from specified index
	const res = regex.exec(input);
	// New longest token
	if (res && res[0].length > len) {
	    len = res[0].length;
	    type = k;
	    value = res[1];
	}
    }
    return {type, value, len};
}

// Create an array that maps from char index to [column, line]
function paint_positions (file) {
    let x=1, y=1;
    return file.split('').map((v) => [x, y] = v == '\n' ? [1, y+1] : [x+1, y]);
}

// Create an object that iterates over tokens
function token_iter (file) {
    let pos = 0;
    const tokens = [];
    const positions = paint_positions(file);
    for (;;) {
	let next = get_token(file, pos);
	if (next.len == 0) break;
	[next.col, next.line] = positions[pos];
	pos += next.len;
	tokens.push(next);
    }
    // TODO: tokens.next() is not a good way of doing this.
    return {
	pos: 0,
	tokens: tokens.filter((t) => t.type != 'Whitespace'),
	get next () { return this.tokens[this.pos++]; },
	get peek () { return this.tokens[this.pos+1]; },
	get current () { return this.tokens[this.pos]; },
	expect: function (...toks) {
	    for (let t of toks) {
		if (this.next.value != t) throw this.error('Expected ' + t);
	    }
	},
	error: function (string) {
	    const c = this.current;
	    return c.line + ':' + c.col + ' (' + c.value + ')\n\t' + string;
	},
    }
}

function parse_file (tok) {
    tok.expect('Entities', '{');
    let ret = parse_exinfo(tok);
    return ret;
}

function parse_entity (tok) {
    const next4floats = () => Array(4).fill().map(() => +tok.next.value);

    // Every entity is formatted in this exact way.
    // This wasn't hardcoded before, but now I just don't care.
    tok.expect('Type', '=');
    let Type = tok.next.value;
    tok.expect('Position', '=', '{');
    let Position = next4floats();
    tok.expect('}');
    tok.expect('Orientation', '=', '{');
    let Orientation = next4floats();
    tok.expect('}');
    tok.expect('ExtraInfo', '{');
    let ExtraInfo = parse_exinfo(tok);
    tok.expect('}');
    return {Type, Position, Orientation, ExtraInfo};
}

function parse_exinfo (tok) {
    const values = [];
    const entities = [];
    while (tok.current.value != '}') {
	const key = tok.next.value;
	if (key == 'Entity') {
	    tok.expect('{');
	    entities.push(parse_entity(tok));
	} else {
	    tok.expect('=');
	    values.push({key, ...parse_value(tok)});
	}
    }
    if (entities.length > 0) {
	// NestedEntities is the placeholder key they use
	// Not sure why the .ini doesn't reflect this
	values.push({key: "NestedEntities", type: "EntityList", value: entities})
    }
    tok.expect('}');
    return values;
}

// Parse values (after =)
function parse_value (tok) {
    const next = tok.next;
    if (next.value == '{') {
	const array = [];
	while (tok.current.value != '}') {
	    array.push(parse_value(tok));
	}
	tok.expect('}');
	return {type: 'List', value: array};
    } else {
	// Values which can be casted
	switch (next.type) {
	    case 'Floating': //fallthrough
	    case 'Integer': next.value = +next.value; break;
	    case 'Bool': next.value = next.value == 'true'; break;
	    case 'Ident': next.type = 'String'; break;
	}
	return {type: next.type, value: next.value};
    }
}

function convert (filename) {
    try {
	let file = fs.readFileSync('maps/' + filename, {encoding: 'UTF-8'});
	const tokens = token_iter(file);
	const data = parse_file(tokens)[0];
	const out = JSON.stringify(data, null, 2)
	// floating hack pt. 2
	    .replace(/"f##([^"]+)"/g, '$1');
	fs.writeFileSync('jsonmaps/' + filename.replace('.ini', '.json'), out, {encoding: 'UTF-8'});
    } catch (err) {
	console.log(filename + ': ' + err);
	throw err;
    }
};

fs.readdir('maps/', (err, files) => {
    if (err) throw 'Maps folder not found!';
    for (let filename of files) convert(filename);
});
