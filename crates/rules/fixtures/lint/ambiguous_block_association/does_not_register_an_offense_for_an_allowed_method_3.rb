expect(order).to receive(:complete).twice { OrderCount.update! }
