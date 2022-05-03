#pragma once

#include "tvrank.h"
#include <QObject>

class TVrankService: public QObject
{
  Q_OBJECT

 public:
  explicit TVrankService(QObject *parent = nullptr);

  TVrankService *service = nullptr;
};
