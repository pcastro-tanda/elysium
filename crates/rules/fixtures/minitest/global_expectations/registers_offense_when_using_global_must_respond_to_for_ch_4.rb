it 'does something' do
  options[:a][:b].must_respond_to 0
  ^^^^^^^^^^^^^^^ Use `value(options[:a][:b])` instead.
end
