expect(JSON.parse(response.body)).to eq('foo' => 'bar')
       ^^^^^^^^^^^^^^^^^^^^^^^^^ Prefer `response.parsed_body`.
