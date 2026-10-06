it 'does something' do
  -> { n }.must_throw 42
  ^^^^^^^^ Use `expect { n }` instead.
end
