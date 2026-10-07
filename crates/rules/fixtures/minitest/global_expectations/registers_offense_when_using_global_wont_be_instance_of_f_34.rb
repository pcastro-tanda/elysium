it 'does something' do
  options[:a][:b].wont_be_instance_of 0
  ^^^^^^^^^^^^^^^ Use `expect(options[:a][:b])` instead.
end
