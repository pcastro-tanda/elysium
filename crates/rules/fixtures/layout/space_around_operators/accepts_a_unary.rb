def bm(label_width = 0, *labels, &blk)
  benchmark(CAPTION, label_width, FORMAT,
            *labels, &blk)
end

def each &block
  +11
end

def self.search *args
end

def each *args
end
