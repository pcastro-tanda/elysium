it 'does something' do
  options[:a][:b].must_be_nil 0
  ^^^^^^^^^^^^^^^ Use `value(options[:a][:b])` instead.
end
