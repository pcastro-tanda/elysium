it 'does something' do
  -> { n }.must_throw 42
  ^^^^^^^^ Use `_ { n }` instead.
end
