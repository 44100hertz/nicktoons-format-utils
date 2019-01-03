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
    String: /"(.*)"/y,
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
	error: function (string) {
	    const c = this.current;
	    return c.line + ':' + c.col + ' (' + c.value + ')\n\t' + string;
	},
    }
}

// Parse a table from the first entry thru the closing brace.
// Watch me de-hardcode this format.
function parse_table (tok, is_exinfo) {
    const values = {};
    const expect = (c) => {
	if (tok.next.value != c) throw tok.error('Expected ' + c);
    }
    while (tok.current != undefined && tok.current.value != '}') {
	const key = tok.next.value;
	if (!is_exinfo && (key == 'Position' || key == 'Orientation')) {
	    expect('=');
	    expect('{');
	    values[key] = [];
	    for (let i=0; i<4; ++i) values[key].push(+tok.next.value);
	    expect('}');
	} else if (!is_exinfo && key == 'Type') { 
	    expect('=');
	    values[key] = tok.next.value;
	} else if (key == 'Entity') {
	    // Turn Entity { A } Entity { B } into
	    // Entities = [{ A }, { B }]
	    expect('{');
	    values.Entities = values.Entities || {List: []};
	    values.Entities.List.push({Entity: parse_table(tok)});
	} else if (key == 'Entities') {
	    // "Entities" only appears at root of file.
	    expect('{');
	    return parse_table(tok);
	} else if (key == 'ExtraInfo') {
	    expect('{');
	    values[key] = parse_table(tok, true);
	} else {
	    expect('=');
	    values[key] = parse_value(tok);
	}
    }
    expect('}');
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
	if (tok.next.value !== '}') throw tok.error('Expected }');
	return {List: array};
    } else {
	// Values which can be casted
	switch (next.type) {
	    case 'Floating': //fallthrough
	    case 'Integer': next.value = +next.value; break;
	    case 'Bool': next.value = next.value == 'true'; break;
	}
	return {[next.type]: next.value};
    }
}

function convert (filename) {
    try {
	let file = fs.readFileSync('maps/' + filename, {encoding: 'UTF-8'});
	const tokens = token_iter(file);
	const data = parse_table(tokens).Entities;
	const out = JSON.stringify(data, null, 4)
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
