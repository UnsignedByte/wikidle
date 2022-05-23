/*
* @Author: UnsignedByte
* @Date:   2022-05-04 21:12:10
* @Last Modified by:   UnsignedByte
* @Last Modified time: 2022-05-21 21:43:52
*/

(() => {
	let guesstable = document.getElementById("guesses");

	let guesses = []

	update(guesses, document.getElementById("guesses"));

	function update (gs, t) {
		gs.sort((a, b) => b.corr-a.corr); // sort by corr value

		// clear table
		t.innerHTML = "";

		t.appendChild(document.createElement("thead"));
		t.appendChild(document.createElement("tbody"));

		// add headr
		let header = t.tHead.insertRow(0);
		header.insertCell(-1).outerHTML = `<th>#</th>`
		header.insertCell(-1).outerHTML = `<th>Guess</th>`
		header.insertCell(-1).outerHTML = `<th>Corr</th>`
		header.insertCell(-1).outerHTML = `<th>Rank</th>`

		t.tHead.insertRow(-1).insertCell(0).outerHTML = "<td colspan=4><hr></td>"


		gs.map(e => {
			let row = t.tBodies[0].insertRow(-1);
			row.insertCell(-1).innerHTML = e.id;
			row.insertCell(-1).innerHTML = e.guess;
			row.insertCell(-1).innerHTML = e.corr.toFixed(5);
			row.insertCell(-1).innerHTML = e.rank;
		})
	}

	function showerror (s) {
		document.getElementById("errormsg").innerHTML = s;
	  var el = document.getElementById('error');
	  el.classList.remove("static");
	  el.style.animation = 'none';
	  el.offsetHeight; /* trigger reflow */
	  el.style.animation = null; 
	}

	document.getElementById("guess-form").addEventListener("submit", e => {
		e.preventDefault();

		let g = document.getElementById("guess");

		let s = g.value.toLowerCase();
		g.value = "";

		console.log(`Guessed ${s}.`)

		fetch(`/api/guess?word=${s}`)
			.then(x=>{
				if (x.status !== 200) {
					x.text().then(showerror)
					return;
				}

				x.json().then(x=> {
					if (guesses.some(x=>x.guess === s)) {
						showerror(`Already guessed ${s}.`);
						return;
					}

					guesses.push({
						guess: s,
						corr: x.corr,
						rank: x.rank,
						id: guesses.length+1
					})

					update(guesses, document.getElementById("guesses"));
				})
		})
		.catch(e => showerror("Failed to fetch from server. Maybe check your internet?"))
	})

})()