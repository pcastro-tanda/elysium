if "foo bar".gsub(/( +)/, "") =~ /foobar/
  p $1
    ^^ $1 is out of range (no regexp capture groups detected).
end
