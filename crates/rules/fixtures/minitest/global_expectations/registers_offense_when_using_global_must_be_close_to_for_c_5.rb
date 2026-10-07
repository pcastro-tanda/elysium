it 'does something' do
  options[:a][:b].must_be_close_to 0
  ^^^^^^^^^^^^^^^ Use `expect(options[:a][:b])` instead.
end
