it 'does something' do
  -> { n }.must_raise 42
  ^^^^^^^^ Use `expect { n }` instead.
end
