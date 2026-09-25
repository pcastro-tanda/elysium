begin
  something
rescue Errno::EAGAIN, Errno::EWOULDBLOCK, Errno::ECONNABORTED
  handle_exception
end
