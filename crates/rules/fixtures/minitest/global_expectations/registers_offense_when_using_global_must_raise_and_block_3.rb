it 'does something' do
  -> { n }.must_raise 42
  ^^^^^^^^ Use `_ { n }` instead.
end
