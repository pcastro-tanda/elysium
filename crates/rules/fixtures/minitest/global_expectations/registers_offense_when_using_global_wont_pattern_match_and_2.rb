it 'does something' do
  -> { n }.wont_pattern_match 42
  ^^^^^^^^ Use `expect { n }` instead.
end
