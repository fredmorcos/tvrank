#pragma once

#include "tvrank.h"
#include <QThread>

class TVrankServiceInit: public QThread
{
  Q_OBJECT

 public:
  explicit TVrankServiceInit(TVrankService& service, QObject *parent = nullptr);

  void run() override;

  Q_SIGNAL void contentLen(const uint64_t len);
  Q_SIGNAL void progress(const uint64_t delta);
  Q_SIGNAL void failed(const enum TVrankServiceError error);
  Q_SIGNAL void success(const TVrankService& service);

 private:
  TVrankService& service;

  static void serviceProgressUpdate(void *data, const uint64_t *contentLen, uint64_t delta);
};
