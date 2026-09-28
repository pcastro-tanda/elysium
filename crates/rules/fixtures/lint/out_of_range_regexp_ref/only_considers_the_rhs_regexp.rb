if "foo bar".gsub(/ +/, "") =~ /foo(bar)/
  p $1
end
