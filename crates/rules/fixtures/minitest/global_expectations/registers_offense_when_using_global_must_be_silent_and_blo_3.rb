it 'does something' do
  -> { n }.must_be_silent 42
  ^^^^^^^^ Use `_ { n }` instead.
end
