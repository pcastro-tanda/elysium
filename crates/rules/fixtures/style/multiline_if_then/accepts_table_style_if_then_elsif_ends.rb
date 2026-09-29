if    @io == $stdout then str << "$stdout"
elsif @io == $stdin  then str << "$stdin"
elsif @io == $stderr then str << "$stderr"
else                      str << @io.class.to_s
end
