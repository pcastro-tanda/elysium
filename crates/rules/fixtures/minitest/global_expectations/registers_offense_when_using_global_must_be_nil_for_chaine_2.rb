it 'does something' do
  options[:a][:b].must_be_nil 0
  ^^^^^^^^^^^^^^^ Use `expect(options[:a][:b])` instead.
end
