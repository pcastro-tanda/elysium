it 'does something' do
  options[:a][:b].must_be_empty 0
  ^^^^^^^^^^^^^^^ Use `expect(options[:a][:b])` instead.
end
